"""Exercise Ctrl-C against a blocked hosted deploy using only a loopback server."""

import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def interrupt(pid):
    if os.name != "nt":
        os.kill(pid, signal.SIGINT)
        return
    # Attach a helper to the child's private console. Never signal the test runner.
    subprocess.run([sys.executable, __file__, "--signal-child", str(pid)], check=True)


def check(binary, phase, reject=False):
    reached = threading.Event()
    release = threading.Event()
    requests = []
    cancel_auth = []
    prefix = "/v0/deployments"
    deployment = prefix + "/dpl_smoke"

    class Api(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def respond(self, status, body):
            data = json.dumps(body).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_POST(self):
            requests.append(self.path)
            if self.path == prefix:
                manifest = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                if phase == "handshake":
                    reached.set()
                    release.wait(15)
                self.respond(200, {"deploymentId": "dpl_smoke", "missing": [manifest["files"][0]["hash"]]})
            elif self.path == deployment + "/cancel":
                cancel_auth.append(self.headers.get("Authorization"))
                self.respond(409 if reject else 200, {"error": "already ready"} if reject else {})
            else:
                self.respond(404, {})

        def do_PUT(self):
            requests.append(self.path)
            reached.set()
            release.wait(15)

    with tempfile.TemporaryDirectory() as root, ThreadingHTTPServer(("127.0.0.1", 0), Api) as api:
        threading.Thread(target=api.serve_forever, daemon=True).start()
        Path(root, "index.html").write_text("<p>Disposable deploy fixture</p>")
        flags = {}
        if os.name == "nt":
            startup = subprocess.STARTUPINFO()
            startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            flags = dict(creationflags=subprocess.CREATE_NEW_CONSOLE, startupinfo=startup)
        env = dict(os.environ, APPZ_API_URL=f"http://127.0.0.1:{api.server_port}",
                   APPZ_TOKEN="fixture", APPZ_PROJECT_ID="fixture", APPZ_VAULT_HOME_OVERRIDE=root)
        env["PATH"] = os.pathsep.join((str(Path(binary).parent), env["PATH"]))
        with subprocess.Popen([binary, "--json", "deploy", "--target", "appzdev", "--no-backup"],
                              cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              **flags) as child:
            try:
                if not reached.wait(15):
                    if child.poll() is not None:
                        _, stderr = child.communicate()
                        raise AssertionError(f"CLI exited before {phase}: {stderr.decode(errors='replace')}")
                    raise AssertionError(f"{phase} never reached (exit {child.poll()})")
                interrupt(child.pid)
                if phase == "handshake":
                    release.set()
                _, stderr = child.communicate(timeout=15)
                assert child.returncode == 130, (child.returncode, stderr)
                assert requests.count(deployment + "/cancel") == 1, requests
                assert deployment + "/commit" not in requests, requests
                assert cancel_auth == ["Bearer fixture"], cancel_auth
                assert (b"already ready" if reject else b"deployment canceled") in stderr, stderr
            finally:
                release.set()
                if child.poll() is None:
                    child.kill()
                api.shutdown()


if __name__ == "__main__":
    if sys.argv[1] == "--signal-child":
        kernel = ctypes.windll.kernel32
        kernel.FreeConsole()
        assert kernel.AttachConsole(int(sys.argv[2]))
        assert kernel.SetConsoleCtrlHandler(None, True)
        assert kernel.GenerateConsoleCtrlEvent(0, 0)
    else:
        executable = str(Path(sys.argv[1]).resolve(strict=True))
        check(executable, "handshake")
        check(executable, "upload")
        check(executable, "upload", reject=True)
        print("Passed handshake interruption, in-flight upload interruption, and cancellation refusal")
