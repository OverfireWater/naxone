# 用 Edge headless + CDP 渲染 HTML 为 PNG，自动按内容真实高度截图（无空白）。
# Usage: .\render.ps1 path\to\input.html path\to\output.png [width]
param([string]$HtmlPath, [string]$OutPath, [int]$Width = 1400)
if (-not $HtmlPath -or -not $OutPath) { throw "Usage: render.ps1 <html> <png> [width]" }

$edge = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"
if (-not (Test-Path $edge)) { throw "找不到 Edge: $edge" }

$port = 9777
$userDir = Join-Path $env:TEMP "edge-cdp-$(Get-Random)"
$htmlAbs = (Resolve-Path $HtmlPath).Path
$fileUrl = "file:///" + ($htmlAbs -replace '\\', '/')

# 先以 headless 模式启 Edge + CDP 端口
$proc = Start-Process -FilePath $edge -ArgumentList @(
  "--headless=new",
  "--disable-gpu",
  "--hide-scrollbars",
  "--no-sandbox",
  "--remote-debugging-port=$port",
  "--user-data-dir=$userDir",
  "--window-size=$Width,2000",
  $fileUrl
) -PassThru -WindowStyle Hidden

Start-Sleep -Seconds 3

try {
  # 拿 page targets
  $targets = Invoke-RestMethod "http://127.0.0.1:$port/json"
  $page = $targets | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
  if (-not $page) { throw "拿不到 page target" }

  $wsUrl = $page.webSocketDebuggerUrl
  Add-Type -AssemblyName System.Net.WebSockets.Client -ErrorAction SilentlyContinue

  $ws = New-Object System.Net.WebSockets.ClientWebSocket
  $cts = New-Object System.Threading.CancellationTokenSource
  $ws.ConnectAsync([uri]$wsUrl, $cts.Token).Wait()

  $msgId = 0
  function Invoke-CDP($method, $params) {
    $script:msgId++
    $payload = @{ id = $script:msgId; method = $method; params = $params } | ConvertTo-Json -Compress -Depth 10
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($payload)
    $segment = [System.ArraySegment[byte]]::new($bytes)
    $ws.SendAsync($segment, [System.Net.WebSockets.WebSocketMessageType]::Text, $true, $cts.Token).Wait()

    # 收响应（可能有事件穿插，循环到拿到对应 id）
    $buf = New-Object byte[] 1048576
    while ($true) {
      $recvSeg = [System.ArraySegment[byte]]::new($buf)
      $result = $ws.ReceiveAsync($recvSeg, $cts.Token)
      $result.Wait()
      $text = [System.Text.Encoding]::UTF8.GetString($buf, 0, $result.Result.Count)
      $obj = $text | ConvertFrom-Json
      if ($obj.id -eq $script:msgId) { return $obj.result }
    }
  }

  # 等页面 load 完
  Start-Sleep -Milliseconds 800

  # 拿 body scrollHeight
  $eval = Invoke-CDP "Runtime.evaluate" @{ expression = "document.documentElement.scrollHeight" }
  $height = [int]$eval.result.value
  Write-Output "Detected content height: ${height}px"

  # 用 captureBeyondViewport 截全图
  $clip = @{ x = 0; y = 0; width = $Width; height = $height; scale = 1 }
  $shot = Invoke-CDP "Page.captureScreenshot" @{
    format = "png"
    clip = $clip
    captureBeyondViewport = $true
  }
  $png = [Convert]::FromBase64String($shot.data)
  [System.IO.File]::WriteAllBytes($OutPath, $png)

  $ws.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "done", $cts.Token).Wait()
  Write-Output "OK: $OutPath  ($Width x $height)"
}
finally {
  Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  Remove-Item -Recurse -Force $userDir -ErrorAction SilentlyContinue
}
