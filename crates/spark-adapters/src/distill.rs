use crate::models::{
    ChatMessage, DistillTask, DistillationRecord, Rollout,
};
use crate::providers::call_provider_unary;
use crate::router::AdapterRouter;
use crate::verifier::Verifier;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct DistillationEngine {
    pub client: Client,
    pub router: AdapterRouter,
    pub dataset_dir: PathBuf,
    pub cortex_endpoint: String,
}

impl DistillationEngine {
    pub fn new() -> Self {
        let default_dir = PathBuf::from("/home/drakestapleton/workspace/distillation-data");
        let _ = create_dir_all(&default_dir);

        Self {
            client: Client::builder().build().unwrap_or_default(),
            router: AdapterRouter::new(),
            dataset_dir: default_dir,
            cortex_endpoint: "http://127.0.0.1:18080".to_string(),
        }
    }

    pub fn with_dataset_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.dataset_dir = dir.as_ref().to_path_buf();
        let _ = create_dir_all(&self.dataset_dir);
        self
    }

    pub async fn distill(&self, task: DistillTask) -> Result<DistillationRecord, String> {
        let mut messages = Vec::new();
        if let Some(ref sys) = task.system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.clone(),
                reasoning: None,
            });
        }
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: task.prompt.clone(),
            reasoning: None,
        });

        // 1. Query Teacher Oracle
        let (teacher_provider, teacher_endpoint, teacher_model_id, teacher_key) =
            self.router.resolve_route(&task.teacher_model);

        let t0 = Instant::now();
        let (teacher_content, teacher_reasoning, teacher_tokens) = call_provider_unary(
            &self.client,
            teacher_provider,
            &teacher_endpoint,
            &teacher_model_id,
            teacher_key.as_deref(),
            &messages,
            Some(0.7),
        )
        .await
        .map_err(|e| format!("Teacher query failed ({}): {}", task.teacher_model, e))?;
        let teacher_duration = t0.elapsed().as_millis() as u64;

        let teacher_verif = Verifier::verify(&teacher_content, task.verification_strategy);

        let teacher_rollout = Rollout {
            provider: teacher_provider,
            model_id: task.teacher_model.clone(),
            content: teacher_content.clone(),
            reasoning: teacher_reasoning.clone(),
            duration_ms: teacher_duration,
            token_count: teacher_tokens,
            verification: teacher_verif.clone(),
        };

        // 2. Query Student (Local Open-Weight Resident Model)
        let mut student_rollout = None;
        let student_target = task.student_model.clone().unwrap_or_else(|| "atlas-lightning-omni".to_string());

        let (student_provider, student_endpoint, student_model_id, student_key) =
            self.router.resolve_route(&student_target);

        let s0 = Instant::now();
        if let Ok((student_content, student_reasoning, student_tokens)) = call_provider_unary(
            &self.client,
            student_provider,
            &student_endpoint,
            &student_model_id,
            student_key.as_deref(),
            &messages,
            Some(0.7),
        )
        .await
        {
            let student_duration = s0.elapsed().as_millis() as u64;
            let student_verif = Verifier::verify(&student_content, task.verification_strategy);

            student_rollout = Some(Rollout {
                provider: student_provider,
                model_id: student_target.clone(),
                content: student_content,
                reasoning: student_reasoning,
                duration_ms: student_duration,
                token_count: student_tokens,
                verification: student_verif,
            });
        }

        // 3. Selection & Preference Logic
        let mut chosen = teacher_content.clone();
        let mut chosen_reasoning = teacher_reasoning.clone();
        let mut rejected = None;
        let mut rejected_reasoning = None;
        let mut preference_delta = 0.0;

        if let Some(ref s_roll) = student_rollout {
            preference_delta = teacher_rollout.verification.score - s_roll.verification.score;
            if teacher_rollout.verification.passed && (!s_roll.verification.passed || teacher_rollout.verification.score >= s_roll.verification.score) {
                chosen = teacher_content.clone();
                chosen_reasoning = teacher_reasoning.clone();
                rejected = Some(s_roll.content.clone());
                rejected_reasoning = s_roll.reasoning.clone();
            } else if s_roll.verification.passed && !teacher_rollout.verification.passed {
                chosen = s_roll.content.clone();
                chosen_reasoning = s_roll.reasoning.clone();
                rejected = Some(teacher_content.clone());
                rejected_reasoning = teacher_reasoning.clone();
            } else {
                chosen = teacher_content.clone();
                rejected = Some(s_roll.content.clone());
            }
        }

        // 4. Persist to SFT and DPO Datasets
        self.persist_training_pair(&task, &chosen, chosen_reasoning.as_deref(), rejected.as_deref())?;

        // 5. Commit Durable Knowledge to Spark Cortex if Verified
        let mut durable_committed = false;
        let mut cortex_id = None;

        if task.commit_to_cortex && teacher_rollout.verification.passed && teacher_rollout.verification.score >= 0.85 {
            let entity_name = format!("distill_lesson:{}_{}", task.id, Utc::now().timestamp());
            let canonical = format!("distill_{}", task.id.replace('-', "_"));
            let content_summary = format!(
                "Distilled from Teacher Oracle {}. Task: {}. Reasoning: {}. Verified Solution: {}",
                task.teacher_model,
                task.prompt,
                teacher_reasoning.as_deref().unwrap_or("<direct>"),
                chosen
            );

            if let Ok(id) = self.commit_to_cortex(&entity_name, &canonical, &content_summary).await {
                durable_committed = true;
                cortex_id = Some(id);
            }
        }

        Ok(DistillationRecord {
            task_id: task.id,
            task_type: task.task_type,
            prompt: task.prompt,
            system_prompt: task.system_prompt,
            teacher_rollout,
            student_rollout,
            chosen,
            chosen_reasoning,
            rejected,
            rejected_reasoning,
            preference_delta,
            durable_memory_committed: durable_committed,
            cortex_entity_id: cortex_id,
            created_at: Utc::now().to_rfc3339(),
        })
    }

    fn persist_training_pair(
        &self,
        task: &DistillTask,
        chosen: &str,
        chosen_reasoning: Option<&str>,
        rejected: Option<&str>,
    ) -> Result<(), String> {
        let _ = create_dir_all(&self.dataset_dir);

        // Append SFT JSONL (ChatML format)
        let sft_file = self.dataset_dir.join("sft.jsonl");
        let mut sft_entry = json!({
            "task_id": task.id,
            "timestamp": Utc::now().to_rfc3339(),
            "messages": [
                { "role": "user", "content": task.prompt }
            ]
        });

        if let Some(ref sys) = task.system_prompt {
            if let Some(arr) = sft_entry.get_mut("messages").and_then(|m| m.as_array_mut()) {
                arr.insert(0, json!({ "role": "system", "content": sys }));
            }
        }

        let mut assistant_obj = json!({
            "role": "assistant",
            "content": chosen
        });
        if let Some(r) = chosen_reasoning {
            assistant_obj["reasoning"] = json!(r);
        }
        sft_entry.get_mut("messages").and_then(|m| m.as_array_mut()).map(|arr| arr.push(assistant_obj));

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&sft_file)
            .map_err(|e| format!("Failed to open sft.jsonl: {}", e))?;
        writeln!(file, "{}", serde_json::to_string(&sft_entry).unwrap())
            .map_err(|e| format!("Failed to write to sft.jsonl: {}", e))?;

        // Append DPO JSONL if rejected pair exists
        if let Some(rej) = rejected {
            let dpo_file = self.dataset_dir.join("dpo.jsonl");
            let dpo_entry = json!({
                "task_id": task.id,
                "prompt": task.prompt,
                "system": task.system_prompt,
                "chosen": chosen,
                "rejected": rej,
                "timestamp": Utc::now().to_rfc3339()
            });

            let mut d_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&dpo_file)
                .map_err(|e| format!("Failed to open dpo.jsonl: {}", e))?;
            writeln!(d_file, "{}", serde_json::to_string(&dpo_entry).unwrap())
                .map_err(|e| format!("Failed to write to dpo.jsonl: {}", e))?;
        }

        Ok(())
    }

    async fn commit_to_cortex(
        &self,
        name: &str,
        canonical_name: &str,
        content: &str,
    ) -> Result<String, String> {
        let token_path = PathBuf::from("/home/drakestapleton/.config/cortex/token");
        let token = std::fs::read_to_string(token_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let url = format!("{}/spaces/atlas-memory/entities", self.cortex_endpoint);
        let payload = json!({
            "name": name,
            "type": "learned_procedure",
            "canonicalName": canonical_name,
            "content": content
        });

        let res = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Cortex request error: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("Cortex returned status {}", res.status()));
        }

        let body: serde_json::Value = res.json().await.unwrap_or_default();
        let entity_id = body.get("id").and_then(|v| v.as_str()).unwrap_or(name).to_string();
        Ok(entity_id)
    }
}
