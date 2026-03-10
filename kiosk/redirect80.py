"""
Port 80 redirect → kiosk on :3300
Pods hitting http://192.168.31.27 get redirected to the kiosk app.
Run as admin (port 80 requires elevated privileges on Windows).
"""
import http.server
import socketserver

REDIRECT_URL = "http://192.168.31.27:3300"


class RedirectHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(302)
        self.send_header("Location", REDIRECT_URL)
        self.end_headers()

    def do_HEAD(self):
        self.do_GET()

    def log_message(self, format, *args):
        pass  # Suppress logs


if __name__ == "__main__":
    with socketserver.TCPServer(("0.0.0.0", 80), RedirectHandler) as httpd:
        print(f"Redirecting :80 → {REDIRECT_URL}")
        httpd.serve_forever()
