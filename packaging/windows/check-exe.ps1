<#
.SYNOPSIS
  Checks that the Windows build really is one file: every DLL it loads must ship with Windows itself.
  Fails when it needs WebView2Loader.dll, the Visual C++ runtime (vcruntime/msvcp/ucrtbase) or anything else that would have to sit beside it.
  Uses llvm-objdump from the Rust toolchain (rustup component add llvm-tools).
#>
param([Parameter(Mandatory)][string]$Exe)
$ErrorActionPreference = 'Stop'
$sysroot = (rustc --print sysroot).Trim()
$objdump = Get-ChildItem -Path (Join-Path $sysroot 'lib/rustlib') -Recurse -Filter 'llvm-objdump.exe' | Select-Object -First 1
if (-not $objdump) { throw 'llvm-objdump not found: rustup component add llvm-tools' }
$dlls = & $objdump.FullName -p $Exe | Select-String -Pattern 'DLL Name:\s*(\S+)' | ForEach-Object { $_.Matches[0].Groups[1].Value.ToLower() } | Sort-Object -Unique
"Needs: " + ($dlls -join ', ')
# Windows' own libraries (API sets included). Anything else would have to be shipped beside the exe.
$system = '^(api-ms-win-.*|ext-ms-.*|kernel32|kernelbase|user32|gdi32|gdiplus|advapi32|shell32|shlwapi|ole32|oleaut32|comctl32|comdlg32|dwmapi|uxtheme|imm32|ws2_32|bcrypt|bcryptprimitives|crypt32|secur32|ntdll|userenv|version|winmm|propsys|rpcrt4|combase|dbghelp|powrprof|setupapi|iphlpapi|oleacc|uiautomationcore|windows.storage|shcore|dxgi|d3d11|d2d1|dwrite)\.dll$'
$extra = $dlls | Where-Object { $_ -notmatch $system }
if ($extra) { throw "The exe needs files that don't come with Windows: $($extra -join ', ')" }
'OK: one file; everything it loads ships with Windows.'
