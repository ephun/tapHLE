# Replay a clickmap: launch an app, drive it through a recorded route, and
# capture a frame after every step.
#
# The point is to stop agents rediscovering the same coordinates. Replay first,
# and spend tokens only from the step where the replay stopped.
#
# This script reports; it does not judge. `expect` is prose and cannot be
# checked mechanically, and comparing frames does not work either -- a screen
# that animates on its own changes whether or not the tap landed, which is the
# false negative recorded in docs/debugging.md. So every step
# gets a capture and a line of output, and a human or an agent looks at them.
#
#   .\dev-scripts\clickmap.ps1 -Map compatibility\clickmaps\cubed-rally-redline.json `
#                              -App "runtime\apps\Cubed Rally Redline (v1.32) [Decrypted].ipa"
#   .\dev-scripts\clickmap.ps1 -Map compatibility\clickmaps\jim-and-frank-hd.json -Validate
#
# Exit code is 0 when every step ran and the app was still alive at the end,
# 1 otherwise. A non-zero exit names the step it stopped on.
#
# See compatibility/clickmaps/protocol.md.

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Map,
    [string]$App,
    [string]$Exe,
    [string]$OutDir,
    [string]$WorkDir,
    # Check the map against the schema's expectations and print the route
    # without launching anything.
    [switch]$Validate,
    # Skip steps whose requires_save_state is "absent" (i.e. the app already
    # has a save). The runner cannot detect this for you.
    [switch]$HasSaveState,
    # Leave the app running when the replay ends, and print its process id.
    #
    # The frames here come from PrintWindow, which asks the window to draw
    # itself into a device context. That is enough for most apps but not for
    # one presenting through OpenGL ES 2.0, where it can return the last
    # composited content rather than the live frame — so a map for such an app
    # says to verify with a screen capture instead. A screen capture needs the
    # window still on screen, which is what this is for. The caller closes the
    # process afterwards.
    [switch]$KeepOpen
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path

if (-not (Test-Path -LiteralPath $Map)) { throw "No such clickmap: $Map" }
$cm = Get-Content -LiteralPath $Map -Raw | ConvertFrom-Json

if ($cm.clickmap_version -ne 1) {
    throw "Clickmap version $($cm.clickmap_version) is not understood by this runner. Refusing to guess."
}
foreach ($field in 'app', 'window', 'milestone', 'steps') {
    if ($null -eq $cm.$field) { throw "Clickmap is missing required field '$field'." }
}

$pressDefault  = if ($cm.defaults -and $cm.defaults.press_ms)  { [int]$cm.defaults.press_ms }  else { 120 }
$settleDefault = if ($cm.defaults -and $cm.defaults.settle_ms) { [int]$cm.defaults.settle_ms } else { 1500 }

function Step-Name($step, $index) {
    if ($step.id) { return $step.id }
    return "step$index"
}

if ($Validate) {
    Write-Output "clickmap : $Map"
    Write-Output "app      : $($cm.app.bundle_identifier) $($cm.app.version)"
    Write-Output "window   : $($cm.window.width)x$($cm.window.height) $($cm.window.orientation)"
    Write-Output "options  : $(if ($cm.options) { $cm.options -join ' ' } else { '(none)' })"
    Write-Output "milestone: $($cm.milestone.rating) star - $($cm.milestone.describes)"
    Write-Output ""
    $i = 0
    foreach ($step in $cm.steps) {
        $i++
        $name = Step-Name $step $i
        $detail = switch ($step.action) {
            'tap'     { "at ($($step.at[0]), $($step.at[1]))" }
            'swipe'   { "($($step.from_xy[0]), $($step.from_xy[1])) -> ($($step.to_xy[0]), $($step.to_xy[1]))" }
            'tilt'    { "held at ($($step.at[0]), $($step.at[1])) for $(if ($step.hold_ms) { $step.hold_ms } else { 400 }) ms" }
            'type'    { "`"$($step.text)`"" }
            'key'     { "scancode $($step.scancode)" }
            'wait'    { "$(if ($step.settle_ms) { $step.settle_ms } else { $settleDefault }) ms" }
            default   { '' }
        }
        $flags = @()
        if ($step.optional) { $flags += 'optional' }
        if ($step.requires_save_state -and $step.requires_save_state -ne 'none') { $flags += "save:$($step.requires_save_state)" }
        $suffix = if ($flags.Count) { "  [$($flags -join ', ')]" } else { '' }
        Write-Output ("{0,-24} {1,-7} {2}{3}" -f $name, $step.action, $detail, $suffix)
        if ($step.expect) { Write-Output ("{0,-24}         -> {1}" -f '', $step.expect) }
    }
    if ($cm.notes) {
        Write-Output ""
        Write-Output "notes:"
        foreach ($n in $cm.notes) { Write-Output "  - $n" }
    }
    exit 0
}

if (-not $App) { throw "-App is required unless -Validate is given." }
# tapHLE runs from $WorkDir, so a path relative to the repo would not resolve
# there. Make it absolute before handing it over.
$App = (Resolve-Path -LiteralPath $App).Path
if (-not $Exe) { $Exe = Join-Path $repo 'target\release\tapHLE.exe' }
if (-not (Test-Path -LiteralPath $Exe)) { throw "No tapHLE binary at $Exe. Build it first." }
if (-not $WorkDir) { $WorkDir = Join-Path $env:TEMP 'taphle-clickmap' }
if (-not $OutDir)  { $OutDir  = Join-Path $WorkDir 'frames' }
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir  | Out-Null

# tapHLE reads default_options.txt from its working directory, so a run
# from anywhere else silently gets no launch options at all. Copy the current
# file in rather than trusting whatever a previous run left behind: a stale copy
# is indistinguishable from a correct one until an app renders wrong.
Copy-Item (Join-Path $repo 'runtime/default_options.txt') (Join-Path $WorkDir 'default_options.txt') -Force
foreach ($link in 'dylibs', 'fonts') {
    $target = Join-Path $repo "runtime/$link"
    $here = Join-Path $WorkDir $link
    if ((Test-Path $target) -and -not (Test-Path $here)) {
        cmd /c mklink /J "`"$here`"" "`"$target`"" | Out-Null
    }
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -Namespace ClickMap -Name Native -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(System.Drawing.Point p);
[DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, System.IntPtr e);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, System.UIntPtr extra);
[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
[DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out System.Drawing.Rectangle r);
[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
'@ -ReferencedAssemblies System.Drawing

$MOUSE_LEFTDOWN = 0x0002
$MOUSE_LEFTUP   = 0x0004
# Tilting is the right button held while the cursor moves: that is what tapHLE
# turns into accelerometer readings (see src/window.rs), and it is the only way
# to steer the many games of this era that are played by tipping the device.
$MOUSE_RIGHTDOWN = 0x0008
$MOUSE_RIGHTUP   = 0x0010
$KEY_KEYUP      = 0x0002
$KEY_SCANCODE   = 0x0008
$VK_MENU        = 0x12
$GA_ROOT        = 2

$argList = @("`"$App`"")
$proc = Start-Process -FilePath $Exe -ArgumentList $argList -WorkingDirectory $WorkDir `
        -RedirectStandardOutput (Join-Path $WorkDir 'run.log') `
        -RedirectStandardError  (Join-Path $WorkDir 'run.err.log') -PassThru
Write-Output "launched pid=$($proc.Id)"

function Get-Hwnd {
    $proc.Refresh()
    if ($proc.HasExited) { return [IntPtr]::Zero }
    return $proc.MainWindowHandle
}

function Client-ToScreen($hwnd, $x, $y) {
    $p = New-Object System.Drawing.Point $x, $y
    [ClickMap.Native]::ClientToScreen($hwnd, [ref]$p) | Out-Null
    return $p
}

# Synthetic input goes wherever the focus and the cursor actually are, not
# where this script meant them to go, so both are checked before anything is
# pressed. SetForegroundWindow cannot be trusted for that: Windows refuses
# foreground changes requested by a process that is not already in front, and
# it refuses by returning true and doing nothing. Tapping ALT lifts that lock.
# The cost of skipping the check is not a bad screenshot -- a replay once put
# a brush stroke on an unsaved document in another application.
function Set-WindowForeground($hwnd) {
    [ClickMap.Native]::keybd_event($VK_MENU, 0, 0, [UIntPtr]::Zero)
    [ClickMap.Native]::keybd_event($VK_MENU, 0, $KEY_KEYUP, [UIntPtr]::Zero)
    [ClickMap.Native]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 120
    return ([ClickMap.Native]::GetForegroundWindow() -eq $hwnd)
}

# A window can be foreground and still not be the thing under the cursor, so
# pointer steps check the point itself. GA_ROOT because the hit test lands on
# whichever child control is there, not on the top-level window.
function Test-PointOverWindow($hwnd, $p) {
    $hit = [ClickMap.Native]::WindowFromPoint($p)
    return ([ClickMap.Native]::GetAncestor($hit, $GA_ROOT) -eq $hwnd)
}

function Capture-Frame($hwnd, $path) {
    $r = New-Object System.Drawing.Rectangle
    [ClickMap.Native]::GetClientRect($hwnd, [ref]$r) | Out-Null
    if ($r.Width -le 0 -or $r.Height -le 0) { return $false }
    $bmp = New-Object System.Drawing.Bitmap ($r.Width + 32), ($r.Height + 64)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dc = $g.GetHdc()
    [ClickMap.Native]::PrintWindow($hwnd, $dc, 0) | Out-Null
    $g.ReleaseHdc($dc); $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return $true
}

$failed = $null
$index = 0
foreach ($step in $cm.steps) {
    $index++
    $name = Step-Name $step $index

    if ($step.requires_save_state -eq 'absent' -and $HasSaveState) {
        Write-Output ("{0,-24} skipped  (needs a fresh profile; -HasSaveState was given)" -f $name)
        continue
    }
    if ($step.requires_save_state -eq 'present' -and -not $HasSaveState) {
        Write-Output ("{0,-24} skipped  (needs an existing save; -HasSaveState was not given)" -f $name)
        continue
    }

    $hwnd = Get-Hwnd
    if ($hwnd -eq [IntPtr]::Zero -and $step.action -ne 'wait') {
        $failed = $name
        Write-Output ("{0,-24} FAILED   no window (app exited?)" -f $name)
        break
    }

    $press  = if ($step.press_ms)  { [int]$step.press_ms }  else { $pressDefault }
    $settle = if ($null -ne $step.settle_ms) { [int]$step.settle_ms } else { $settleDefault }

    switch ($step.action) {
        'wait' { }
        'capture' { }
        'tap' {
            if (-not (Set-WindowForeground $hwnd)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   the app never came to the front" -f $name)
                break
            }
            $p = Client-ToScreen $hwnd ([int]$step.at[0]) ([int]$step.at[1])
            [ClickMap.Native]::SetCursorPos($p.X, $p.Y) | Out-Null
            Start-Sleep -Milliseconds 60
            if (-not (Test-PointOverWindow $hwnd $p)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   another window is at ({1}, {2}); not clicking" -f $name, $p.X, $p.Y)
                break
            }
            [ClickMap.Native]::mouse_event($MOUSE_LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
            Start-Sleep -Milliseconds $press
            [ClickMap.Native]::mouse_event($MOUSE_LEFTUP, 0, 0, 0, [IntPtr]::Zero)
        }
        'swipe' {
            if (-not (Set-WindowForeground $hwnd)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   the app never came to the front" -f $name)
                break
            }
            $a = Client-ToScreen $hwnd ([int]$step.from_xy[0]) ([int]$step.from_xy[1])
            $b = Client-ToScreen $hwnd ([int]$step.to_xy[0])   ([int]$step.to_xy[1])
            $ms = if ($step.duration_ms) { [int]$step.duration_ms } else { 400 }
            $frames = [Math]::Max(2, [int]($ms / 16))
            [ClickMap.Native]::SetCursorPos($a.X, $a.Y) | Out-Null
            Start-Sleep -Milliseconds 60
            if (-not (Test-PointOverWindow $hwnd $a)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   another window is at ({1}, {2}); not dragging" -f $name, $a.X, $a.Y)
                break
            }
            [ClickMap.Native]::mouse_event($MOUSE_LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
            for ($f = 1; $f -le $frames; $f++) {
                $t = $f / $frames
                $x = [int]($a.X + ($b.X - $a.X) * $t)
                $y = [int]($a.Y + ($b.Y - $a.Y) * $t)
                [ClickMap.Native]::SetCursorPos($x, $y) | Out-Null
                Start-Sleep -Milliseconds 16
            }
            [ClickMap.Native]::mouse_event($MOUSE_LEFTUP, 0, 0, 0, [IntPtr]::Zero)
        }
        'tilt' {
            # A tilt is a position, not a movement. tapHLE turns where the
            # cursor is while the right button is down into how far the device
            # is tipped -- the middle of the window is level and the edges are
            # full tilt -- so steering means holding the button at a point, and
            # `hold_ms` is how long the game is given to move under that tilt.
            # Letting go levels the device again.
            if (-not (Set-WindowForeground $hwnd)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   the app never came to the front" -f $name)
                break
            }
            $p = Client-ToScreen $hwnd ([int]$step.at[0]) ([int]$step.at[1])
            $hold = if ($step.hold_ms) { [int]$step.hold_ms } else { 400 }
            [ClickMap.Native]::SetCursorPos($p.X, $p.Y) | Out-Null
            Start-Sleep -Milliseconds 60
            if (-not (Test-PointOverWindow $hwnd $p)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   another window is at ({1}, {2}); not tilting" -f $name, $p.X, $p.Y)
                break
            }
            [ClickMap.Native]::mouse_event($MOUSE_RIGHTDOWN, 0, 0, 0, [IntPtr]::Zero)
            # Nudged once inside the hold: the tilt arrives as an event, and a
            # press with no motion after it is one event at the start rather
            # than a device being held over.
            Start-Sleep -Milliseconds ([Math]::Max(1, [int]($hold / 2)))
            [ClickMap.Native]::SetCursorPos($p.X, $p.Y) | Out-Null
            Start-Sleep -Milliseconds ([Math]::Max(1, [int]($hold / 2)))
            [ClickMap.Native]::mouse_event($MOUSE_RIGHTUP, 0, 0, 0, [IntPtr]::Zero)
        }
        'type' {
            if (-not (Set-WindowForeground $hwnd)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   the app never came to the front" -f $name)
                break
            }
            [System.Windows.Forms.SendKeys]::SendWait($step.text)
        }
        'key' {
            # A scancode, not a virtual key: SDL does not see a virtual-key-only
            # synthetic press, which is why maps record scancodes.
            if (-not (Set-WindowForeground $hwnd)) {
                $failed = $name
                Write-Output ("{0,-24} FAILED   the app never came to the front" -f $name)
                break
            }
            $sc = [byte][int]$step.scancode
            [ClickMap.Native]::keybd_event(0, $sc, $KEY_SCANCODE, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds 60
            [ClickMap.Native]::keybd_event(0, $sc, ($KEY_SCANCODE -bor $KEY_KEYUP), [UIntPtr]::Zero)
        }
        default {
            $failed = $name
            Write-Output ("{0,-24} FAILED   unknown action '{1}'" -f $name, $step.action)
            break
        }
    }
    if ($failed) { break }

    $waited = 0
    while ($waited -lt $settle) {
        Start-Sleep -Milliseconds ([Math]::Min(500, $settle - $waited))
        $waited += 500
    }

    $proc.Refresh()
    if ($proc.HasExited) {
        Write-Output ("{0,-24} FAILED   app exited during/after this step" -f $name)
        $failed = $name
        break
    }

    $shot = Join-Path $OutDir ("{0:d2}-{1}.png" -f $index, $name)
    $hwnd = Get-Hwnd
    $ok = $false
    if ($hwnd -ne [IntPtr]::Zero) { $ok = Capture-Frame $hwnd $shot }
    $note = if ($step.expect) { " -> $($step.expect)" } else { '' }
    $cap = if ($ok) { [IO.Path]::GetFileName($shot) } else { 'no frame' }
    Write-Output ("{0,-24} ok       {1}{2}" -f $name, $cap, $note)
}

if (-not $failed) {
    $proc.Refresh()
    if ($proc.HasExited) { $failed = '(after last step)' }
}

Write-Output ""
Write-Output "frames: $OutDir"
Write-Output "log   : $(Join-Path $WorkDir 'run.err.log')"

if ($failed) {
    Write-Output "RESULT: stopped at '$failed'. The recorded milestone was $($cm.milestone.rating) star."
    Write-Output "        Look at the last frame, then explore from there. If this map is"
    Write-Output "        recorded at a rating the app no longer reaches, that is a regression"
    Write-Output "        and gets a report naming this step."
    if ($KeepOpen) {
        Write-Output "        Left running as pid $($proc.Id); close it when you are done."
    } else {
        try { $proc.Kill() } catch { }
    }
    exit 1
}

Write-Output "RESULT: replayed to the end. Recorded milestone: $($cm.milestone.rating) star - $($cm.milestone.describes)"
Write-Output "        Check the last frames against that description before claiming it."
if ($KeepOpen) {
    Write-Output "        Left running as pid $($proc.Id); close it when you are done."
} else {
    try { $proc.Kill() } catch { }
}
exit 0
