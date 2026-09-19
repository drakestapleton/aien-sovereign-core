---
name: gpu-telemetry
description: Query, parse, and stream NVIDIA DGX Spark Grace Blackwell GB10 GPU metrics, thermals, VRAM, and NVLink telemetry.
---

# GPU Telemetry Protocol (Grace Blackwell GB10)

Use this skill when diagnosing GPU performance, monitoring training or inference workloads, verifying VRAM headroom, or building telemetry dashboards on the NVIDIA DGX Spark workstation.

## Hardware Context

The Spark workstation features:
- **NVIDIA Grace Blackwell GB10 Architecture**
- Unified Coherent Memory between Grace CPU and Blackwell GPU (High-bandwidth NVLink-C2C)
- Unified physical memory pool with dynamic GPU memory allocation

## Standard Query Commands

### 1. High-Density Snapshot
```bash
nvidia-smi --query-gpu=name,temperature.gpu,utilization.gpu,utilization.memory,memory.total,memory.used,memory.free,power.draw,clocks.current.sm --format=csv,noheader,nounits
```

### 2. Formatted Table View
```bash
nvidia-smi --format=csv --query-gpu=timestamp,name,pstate,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw
```

### 3. Grace CPU & Coherent Fabric Telemetry
Inspect Grace Neoverse V2 CPU core utilization and NVLink metrics:
```bash
cat /proc/driver/nvidia/gpus/*/information
```

## Parsing Guidelines for Tools & Dashboards

When parsing CSV output:
- Values are comma-delimited with spaces trimmed.
- Temperatures above 82°C indicate thermal throttling risk.
- VRAM usage should be tracked against the unified address space.
