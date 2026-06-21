# tools/dev-echo.ps1
# Human: Local smoke — start server, feed bridge one handshake + ping via stdin.
$ErrorActionPreference = "Stop"

# Start the server in a separate process; capture it so we can stop it later.
$server = Start-Process -FilePath "cargo" -ArgumentList "run","-p","x4mp-server" -PassThru
Start-Sleep -Seconds 2

# The bridge performs the handshake itself on connect; stdin carries game events only.
$ping = '{"v":1,"type":"session.ping","session_id":"00000000-0000-0000-0000-000000000001","seq":0,"timestamp_sim":0.0,"sender_id":"00000000-0000-0000-0000-000000000002","authority":"client","payload":{}}'

try {
    @($ping) | cargo run -p x4mp-bridge
}
finally {
    Stop-Process -Id $server.Id -ErrorAction SilentlyContinue
}
