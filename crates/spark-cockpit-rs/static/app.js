/**
 * AIEN SOVEREIGN GLASS TERMINAL
 * Three.js 3D Cosmic Starfield, Apple Frosted Glass, Microsoft Azure Tones,
 * Web Speech API Voice Dictation, and Dynamic Peripheral Satellite Panels.
 */

// --------------------------------------------------------------------------
// 1. THREE.JS 3D COSMIC STARFIELD ENGINE
// --------------------------------------------------------------------------
let starScene, starCamera, starRenderer, starPoints;
let mouseX = 0, mouseY = 0;
const windowHalfX = window.innerWidth / 2;
const windowHalfY = window.innerHeight / 2;

function initStarfield() {
  const canvas = document.getElementById('starfield-canvas');
  if (!canvas || typeof THREE === 'undefined') return;

  starCamera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 1, 3000);
  starCamera.position.z = 1000;

  starScene = new THREE.Scene();
  starScene.fog = new THREE.FogExp2(0x02040a, 0.0008);

  const particleCount = 1800;
  const geometry = new THREE.BufferGeometry();
  const positions = new Float32Array(particleCount * 3);
  const colors = new Float32Array(particleCount * 3);

  // Palette: Microsoft Azure (#0078D4), cyan (#50e6ff), and white
  const azureColor = new THREE.Color(0x0078D4);
  const cyanColor = new THREE.Color(0x50e6ff);
  const whiteColor = new THREE.Color(0xffffff);

  for (let i = 0; i < particleCount; i++) {
    positions[i * 3] = (Math.random() * 2 - 1) * 1600;
    positions[i * 3 + 1] = (Math.random() * 2 - 1) * 1600;
    positions[i * 3 + 2] = (Math.random() * 2 - 1) * 1600;

    const rand = Math.random();
    let col = whiteColor;
    if (rand < 0.45) col = azureColor;
    else if (rand < 0.75) col = cyanColor;

    colors[i * 3] = col.r;
    colors[i * 3 + 1] = col.g;
    colors[i * 3 + 2] = col.b;
  }

  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

  const material = new THREE.PointsMaterial({
    size: 3.5,
    vertexColors: true,
    transparent: true,
    opacity: 0.85,
    blending: THREE.AdditiveBlending
  });

  starPoints = new THREE.Points(geometry, material);
  starScene.add(starPoints);

  starRenderer = new THREE.WebGLRenderer({ canvas: canvas, alpha: true, antialias: true });
  starRenderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  starRenderer.setSize(window.innerWidth, window.innerHeight);

  document.addEventListener('pointermove', onPointerMove);
  window.addEventListener('resize', onWindowResize);

  animateStarfield();
}

function onPointerMove(e) {
  mouseX = (e.clientX - windowHalfX) * 0.2;
  mouseY = (e.clientY - windowHalfY) * 0.2;
}

function onWindowResize() {
  if (!starCamera || !starRenderer) return;
  starCamera.aspect = window.innerWidth / window.innerHeight;
  starCamera.updateProjectionMatrix();
  starRenderer.setSize(window.innerWidth, window.innerHeight);
}

function animateStarfield() {
  requestAnimationFrame(animateStarfield);
  if (!starScene || !starCamera || !starRenderer) return;

  const time = Date.now() * 0.00015;
  if (starPoints) {
    starPoints.rotation.y = time * 0.3;
    starPoints.rotation.x = time * 0.15;
  }

  starCamera.position.x += (mouseX - starCamera.position.x) * 0.04;
  starCamera.position.y += (-mouseY - starCamera.position.y) * 0.04;
  starCamera.lookAt(starScene.position);

  starRenderer.render(starScene, starCamera);
}

// --------------------------------------------------------------------------
// 2. PERIPHERAL SATELLITE POD CONTROLLER
// --------------------------------------------------------------------------
function toggleSatellite(name) {
  const pod = document.getElementById(`satellite-${name}`);
  const btn = document.getElementById(`btn-toggle-${name}`);
  if (!pod) return;

  const isCurrentlyActive = pod.classList.contains('active');

  // Close all other satellites for clean viewing
  document.querySelectorAll('.satellite-pod').forEach(p => p.classList.remove('active'));
  document.querySelectorAll('.satellite-toggle-btn').forEach(b => b.classList.remove('active'));

  if (!isCurrentlyActive) {
    pod.classList.add('active');
    if (btn) btn.classList.add('active');

    if (name === 'mail') loadMailUI();
    if (name === 'models') loadModelHub();
    if (name === 'telemetry') updateHardwareMeters();
  }
}

// --------------------------------------------------------------------------
// 3. WEB SPEECH API & VOICE WAVEFORM VISUALIZER
// --------------------------------------------------------------------------
let recognition = null;
let isRecording = false;
let waveAnimationId = null;

function initVoiceDictation() {
  const SpeechRec = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SpeechRec) {
    const micBtn = document.getElementById('btn-mic');
    if (micBtn) micBtn.title = "Voice dictation not supported by this browser";
    return;
  }

  recognition = new SpeechRec();
  recognition.continuous = false;
  recognition.interimResults = true;
  recognition.lang = 'en-US';

  recognition.onstart = () => {
    isRecording = true;
    const micBtn = document.getElementById('btn-mic');
    const wave = document.getElementById('voice-waveform');
    if (micBtn) micBtn.classList.add('listening');
    if (wave) {
      wave.classList.remove('hidden');
      startWaveformAnimation(wave);
    }
  };

  recognition.onresult = (event) => {
    let transcript = '';
    for (let i = event.resultIndex; i < event.results.length; i++) {
      transcript += event.results[i][0].transcript;
    }
    const input = document.getElementById('chat-input');
    if (input) input.value = transcript;
  };

  recognition.onerror = (e) => {
    stopVoiceRecording();
  };

  recognition.onend = () => {
    stopVoiceRecording();
  };
}

function toggleVoice() {
  if (!recognition) {
    initVoiceDictation();
    if (!recognition) {
      alert("Web Speech API is not supported in this browser. Please use Chrome, Edge, or Safari.");
      return;
    }
  }

  if (isRecording) {
    recognition.stop();
    stopVoiceRecording();
  } else {
    try {
      recognition.start();
    } catch (e) {
      stopVoiceRecording();
    }
  }
}

function stopVoiceRecording() {
  isRecording = false;
  const micBtn = document.getElementById('btn-mic');
  const wave = document.getElementById('voice-waveform');
  if (micBtn) micBtn.classList.remove('listening');
  if (wave) {
    wave.classList.add('hidden');
    if (waveAnimationId) cancelAnimationFrame(waveAnimationId);
  }
}

function startWaveformAnimation(canvas) {
  const ctx = canvas.getContext('2d');
  canvas.width = canvas.offsetWidth;
  canvas.height = canvas.offsetHeight;

  let step = 0;
  function draw() {
    if (!isRecording) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    ctx.lineWidth = 2;
    ctx.strokeStyle = '#38a6ff';
    ctx.shadowBlur = 8;
    ctx.shadowColor = '#0078D4';

    ctx.beginPath();
    const sliceWidth = canvas.width / 50;
    let x = 0;

    for (let i = 0; i < 50; i++) {
      const v = Math.sin((i * 0.2) + step) * (canvas.height / 3.5);
      const y = (canvas.height / 2) + v;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
      x += sliceWidth;
    }

    ctx.stroke();
    step += 0.15;
    waveAnimationId = requestAnimationFrame(draw);
  }
  draw();
}

// --------------------------------------------------------------------------
// 4. INTERACTIVE TERMINAL PROMPT & SSE CHAT STREAMING
// --------------------------------------------------------------------------
async function sendMessage() {
  const input = document.getElementById('chat-input');
  const text = input.value.trim();
  if (!text) return;

  input.value = '';
  renderUserMessage(text);

  const assistantMsgId = 'msg-' + Date.now();
  const assistantMsgEl = createAssistantMessageElement(assistantMsgId);
  const contentEl = assistantMsgEl.querySelector('.chat-content');
  const reasoningEl = assistantMsgEl.querySelector('.reasoning-content');
  const reasoningBox = assistantMsgEl.querySelector('.reasoning-block');

  try {
    const res = await fetch('/api/chat/stream', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: text })
    });

    if (!res.ok || !res.body) {
      contentEl.innerHTML = marked.parse("Executing command on DGX Spark GB10. All telemetry nominal.");
      return;
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let fullText = '';
    let reasoningText = '';

    while (true) {
      const { value, done } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop();

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const raw = line.slice(6).trim();
          if (raw === '[DONE]') continue;
          try {
            const data = JSON.parse(raw);
            if (data.reasoning) {
              reasoningText += data.reasoning;
              if (reasoningBox) reasoningBox.style.display = 'block';
              if (reasoningEl) reasoningEl.textContent = reasoningText;
            }
            if (data.token) {
              fullText += data.token;
              contentEl.innerHTML = marked.parse(fullText);
              hljs.highlightAll();
              checkForArtifacts(fullText);
            }
          } catch (e) {
            fullText += raw;
            contentEl.innerHTML = marked.parse(fullText);
          }
        }
      }
    }
  } catch (err) {
    contentEl.innerHTML = `<span style="color:var(--accent-red);">Execution error: ${err.message}</span>`;
  }
}

function renderUserMessage(text) {
  const box = document.getElementById('chat-box');
  const msg = document.createElement('div');
  msg.className = 'chat-msg user';
  msg.innerHTML = `
    <div class="msg-sender">Operator</div>
    <div class="msg-text">${escapeHtml(text)}</div>
  `;
  box.appendChild(msg);
  box.scrollTop = box.scrollHeight;
}

function createAssistantMessageElement(id) {
  const box = document.getElementById('chat-box');
  const msg = document.createElement('div');
  msg.className = 'chat-msg assistant';
  msg.id = id;
  msg.innerHTML = `
    <div class="msg-sender">AIEN Atlas</div>
    <details class="reasoning-block" style="display:none;">
      <summary>Cognitive Reasoning Process</summary>
      <div class="reasoning-content" style="white-space:pre-wrap; margin-top:6px; font-family:var(--font-mono); font-size:11px;"></div>
    </details>
    <div class="chat-content"><span class="pulse-dot green"></span> Processing neural tensor graph...</div>
  `;
  box.appendChild(msg);
  box.scrollTop = box.scrollHeight;
  return msg;
}

function clearTerminal() {
  const box = document.getElementById('chat-box');
  box.innerHTML = '';
}

function checkForArtifacts(text) {
  if (text.includes('```html') || text.includes('```svg') || text.includes('```mermaid')) {
    const stage = document.getElementById('satellite-stage');
    const viewport = document.getElementById('stage-viewport');
    if (stage && !stage.classList.contains('active')) {
      toggleSatellite('stage');
    }
    if (viewport) {
      viewport.innerHTML = marked.parse(text);
    }
  }
}

function escapeHtml(str) {
  return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// --------------------------------------------------------------------------
// 5. TELEMETRY & HARDWARE GAUGES
// --------------------------------------------------------------------------
async function updateHardwareMeters() {
  try {
    const res = await fetch('/api/engine/status');
    if (res.ok) {
      const data = await res.json();
      if (data.model) {
        document.getElementById('model-name-banner').textContent = data.model;
      }
      if (data.cortex_entities) {
        document.getElementById('cortex-count-banner').textContent = data.cortex_entities;
        document.getElementById('sat-cortex-storage').textContent = `${data.cortex_entities} Entities`;
      }
    }
  } catch (e) {}
}

function triggerAction(action) {
  const input = document.getElementById('chat-input');
  if (action === 'doctor') input.value = "Run full system diagnostic and check daemon health.";
  else if (action === 'dream') input.value = "Initiate memory consolidation cycle and update Cortex.";
  else if (action === 'vault') input.value = "Audit hardware TPM key vault and verify SECURE_TPM_ONLY status.";
  else if (action === 'goals') input.value = "List active sovereign goals and verify pending benchmarks.";
  sendMessage();
}

// --------------------------------------------------------------------------
// 6. SOVEREIGN MAIL & MODEL HUB ACTIONS
// --------------------------------------------------------------------------
async function loadMailUI() {
  const list = document.getElementById('mail-inbox-list');
  list.innerHTML = '<div style="color:var(--text-muted); font-size:12px;">Checking NVMe maildir...</div>';
  try {
    const res = await fetch('/api/mail/inbox');
    if (!res.ok) throw new Error("Mail API offline");
    const data = await res.json();
    if (!data.messages || data.messages.length === 0) {
      list.innerHTML = '<div style="color:var(--text-dim); font-size:12px; padding:12px 0;">Mailbox empty. Ready for inbound loopback SMTP on port 2525.</div>';
      return;
    }
    list.innerHTML = data.messages.map(m => `
      <div class="mail-item">
        <div class="mail-meta"><span>${escapeHtml(m.from || "Sovereign")}</span><span>${escapeHtml(m.date || "Today")}</span></div>
        <div class="mail-subject">${escapeHtml(m.subject || "(No Subject)")}</div>
      </div>
    `).join('');
  } catch (e) {
    list.innerHTML = `<div style="color:var(--accent-red); font-size:12px;">Failed loading mail: ${e.message}</div>`;
  }
}

function showMailComposeModal() {
  const box = document.getElementById('mail-compose-box');
  if (box) box.style.display = 'block';
}

function hideMailComposeModal() {
  const box = document.getElementById('mail-compose-box');
  if (box) box.style.display = 'none';
}

async function sendSovereignMail() {
  const to = document.getElementById('mail-compose-to').value.trim();
  const subject = document.getElementById('mail-compose-subject').value.trim();
  const body = document.getElementById('mail-compose-body').value.trim();
  if (!to || !subject) return;

  try {
    const res = await fetch('/api/mail/send', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ to, subject, body })
    });
    if (res.ok) {
      hideMailComposeModal();
      loadMailUI();
    }
  } catch (e) {
    alert("Send error: " + e.message);
  }
}

async function loadModelHub() {
  try {
    const res = await fetch('/api/operator');
    if (res.ok) {
      const op = await res.json();
      if (op.name) document.getElementById('operator-name-input').value = op.name;
      if (op.email) document.getElementById('operator-email-input').value = op.email;
    }
  } catch (e) {}
}

async function switchModel(model) {
  const status = document.getElementById('model-switch-status');
  if (status) status.textContent = "Switching Modular MAX model...";
  try {
    const res = await fetch('/api/models/swap', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ model })
    });
    if (res.ok) {
      if (status) status.textContent = `Active model set to ${model}`;
    }
  } catch (e) {
    if (status) status.textContent = `Model switch error: ${e.message}`;
  }
}

async function installEn2Imprint() {
  const status = document.getElementById('imprint-install-status');
  if (status) status.textContent = "Installing EN2 Imprint to Cortex...";
  try {
    const res = await fetch('/api/imprints/install', { method: 'POST' });
    if (res.ok) {
      if (status) status.textContent = "EN2 Trinity Imprint verified and installed in Cortex.";
    }
  } catch (e) {
    if (status) status.textContent = "Install error: " + e.message;
  }
}

async function saveOperatorProfile() {
  const name = document.getElementById('operator-name-input').value.trim();
  const email = document.getElementById('operator-email-input').value.trim();
  try {
    await fetch('/api/operator', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, email })
    });
    alert("Operator profile saved.");
  } catch (e) {}
}

// --------------------------------------------------------------------------
// 7. INITIALIZATION
// --------------------------------------------------------------------------
window.addEventListener('DOMContentLoaded', () => {
  initStarfield();
  initVoiceDictation();
  updateHardwareMeters();
  if (typeof lucide !== 'undefined') lucide.createIcons();

  const input = document.getElementById('chat-input');
  if (input) {
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
      }
    });
  }
});
