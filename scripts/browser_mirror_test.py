"""Sovereign Browser Mirror Self-Testing Harness for AIEN."""

import asyncio
import base64
import json
import os
import signal
import subprocess
import time
import urllib.request
import websockets

TARGET_URL = "http://127.0.0.1:18095/"
CDP_HOST = "127.0.0.1"
CDP_PORT = 9222
OUT_DIR = "/home/drakestapleton/basecamp/ui-tests"

os.makedirs(OUT_DIR, exist_ok=True)

class CDPClient:
    def __init__(self, ws_url):
        self.ws_url = ws_url
        self.ws = None
        self.req_id = 1

    async def connect(self):
        self.ws = await websockets.connect(self.ws_url, max_size=50 * 1024 * 1024)

    async def send(self, method, params=None):
        mid = self.req_id
        self.req_id += 1
        msg = {"id": mid, "method": method}
        if params:
            msg["params"] = params
        await self.ws.send(json.dumps(msg))
        while True:
            resp = json.loads(await self.ws.recv())
            if resp.get("id") == mid:
                return resp.get("result", {})

    async def eval_js(self, expr, await_promise=False):
        res = await self.send("Runtime.evaluate", {
            "expression": expr,
            "returnByValue": True,
            "awaitPromise": await_promise
        })
        val = res.get("result", {}).get("value")
        return val

    async def capture_screenshot(self, filename):
        path = os.path.join(OUT_DIR, filename)
        res = await self.send("Page.captureScreenshot", {"format": "png"})
        data = res.get("data")
        if data:
            with open(path, "wb") as f:
                f.write(base64.b64decode(data))
            print(f"[mirror] Saved screenshot: {path}")
            return path
        return None

    async def close(self):
        if self.ws:
            await self.ws.close()


async def run_mirror_test():
    report = {
        "timestamp": time.time(),
        "target": TARGET_URL,
        "assertions": [],
        "screenshots": [],
        "status": "in_progress"
    }

    # 1. Launch Chrome Headless
    chrome_cmd = [
        "google-chrome",
        "--headless",
        f"--remote-debugging-port={CDP_PORT}",
        "--disable-gpu",
        "--no-sandbox",
        "--window-size=1280,900",
        TARGET_URL
    ]
    print(f"[mirror] Spawning headless Chrome on port {CDP_PORT}")
    proc = subprocess.Popen(chrome_cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    try:
        ws_url = None
        for _ in range(30):
            try:
                with urllib.request.urlopen(f"http://{CDP_HOST}:{CDP_PORT}/json") as r:
                    pages = json.loads(r.read().decode())
                    for p in pages:
                        if p.get("type") == "page" and "18095" in p.get("url", ""):
                            ws_url = p.get("webSocketDebuggerUrl")
                            break
                if ws_url:
                    break
            except Exception:
                pass
            await asyncio.sleep(0.2)

        if not ws_url:
            raise RuntimeError("Failed to obtain Chrome CDP WebSocketDebuggerUrl")

        print(f"[mirror] Connected to CDP session: {ws_url}")
        client = CDPClient(ws_url)
        await client.connect()

        await client.send("Page.enable")
        await client.send("Runtime.enable")
        await client.send("DOM.enable")

        # Wait for hydration
        await asyncio.sleep(2.0)

        # Assertion 1: Title and Header Telemetry
        title = await client.eval_js("document.title")
        bpm = await client.eval_js("document.getElementById('badge-heartbeat-text')?.textContent?.trim()")
        coherence = await client.eval_js("document.getElementById('badge-coherence-text')?.textContent?.trim()")
        print(f"[mirror] Page Title: '{title}', BPM: '{bpm}', Coherence: '{coherence}'")

        assert_title = "AIEN" in (title or "")
        report["assertions"].append({
            "name": "initial_page_hydration",
            "passed": assert_title,
            "title": title,
            "bpm": bpm,
            "coherence": coherence
        })

        p1 = await client.capture_screenshot("cockpit_init.png")
        if p1:
            report["screenshots"].append(p1)

        # Assertion 2: Chat input and live streaming response
        prompt = "Hello AIEN, please report current sovereign status."
        print(f"[mirror] Submitting Chat message: '{prompt}'")
        js_send = f"""
        document.getElementById('chat-input').value = {json.dumps(prompt)};
        sendMessage();
        """
        await client.eval_js(js_send)

        # Poll for stream completion
        response_text = ""
        for i in range(50):
            await asyncio.sleep(0.5)
            is_streaming = await client.eval_js("window.isStreaming")
            msgs = await client.eval_js("""
                Array.from(document.querySelectorAll('.chat-msg.assistant')).map(el => el.textContent.trim())
            """)
            if msgs and len(msgs) > 1:
                last_msg = msgs[-1]
                if len(last_msg) > 30 and (is_streaming is False or i > 25):
                    response_text = last_msg
                    break

        print(f"[mirror] Assistant Response Length: {len(response_text)} chars")
        has_response = len(response_text) > 20
        report["assertions"].append({
            "name": "chat_live_stream",
            "passed": has_response,
            "response_snippet": response_text[:180]
        })

        p2 = await client.capture_screenshot("cockpit_chat_stream.png")
        if p2:
            report["screenshots"].append(p2)

        # Assertion 3: Tactile Action Pill
        print("[mirror] Clicking 'Run Doctor' tactile button")
        await client.eval_js("""
            const pills = Array.from(document.querySelectorAll('.action-pill'));
            const docPill = pills.find(p => p.textContent.includes('Doctor'));
            if (docPill) docPill.click();
        """)
        await asyncio.sleep(1.5)

        # Assertion 4: Tab Navigation
        print("[mirror] Switching to Goals tab")
        await client.eval_js("""
            const btn = document.querySelector('button[data-tab="goals"]');
            if (btn) btn.click();
        """)
        await asyncio.sleep(1.0)
        goals_content = await client.eval_js("document.getElementById('pane-goals')?.textContent?.trim()")
        has_goals = len(goals_content or "") > 20

        print("[mirror] Switching to Skills tab")
        await client.eval_js("""
            const btn = document.querySelector('button[data-tab="skills"]');
            if (btn) btn.click();
        """)
        await asyncio.sleep(1.0)
        skills_content = await client.eval_js("document.getElementById('pane-skills')?.textContent?.trim()")
        has_skills = len(skills_content or "") > 20

        print(f"[mirror] Goals Tab Content: {has_goals}, Skills Tab Content: {has_skills}")

        report["assertions"].append({
            "name": "tab_navigation_and_actions",
            "passed": has_goals and has_skills,
            "has_goals": has_goals,
            "has_skills": has_skills
        })

        p3 = await client.capture_screenshot("cockpit_actions.png")
        if p3:
            report["screenshots"].append(p3)

        all_passed = all(a.get("passed", False) for a in report["assertions"])
        report["status"] = "PASSED" if all_passed else "FAILED"
        print(f"[mirror] Mirror Self-Test Result: {report['status']}")

        await client.close()
    finally:
        try:
            proc.terminate()
            proc.wait(timeout=3)
        except Exception:
            proc.kill()

    report_path = os.path.join(OUT_DIR, "report.json")
    with open(report_path, "w") as f:
        json.dump(report, f, indent=2)
    print(f"[mirror] Test Report written to: {report_path}")
    return report


if __name__ == "__main__":
    rep = asyncio.run(run_mirror_test())
    print("\n" + json.dumps(rep, indent=2))
