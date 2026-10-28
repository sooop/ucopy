# 탐색기 '보내기' 메뉴에 '내용 복사' 바로가기를 등록/해제한다.
#   설치:  powershell -ExecutionPolicy Bypass -File install.ps1
#   해제:  powershell -ExecutionPolicy Bypass -File install.ps1 -Uninstall
param([switch]$Uninstall)

$sendTo = [Environment]::GetFolderPath('SendTo')
$lnk = Join-Path $sendTo 'CopyContents.lnk'

if ($Uninstall) {
    Remove-Item -LiteralPath $lnk -ErrorAction SilentlyContinue
    Write-Host "제거: $lnk"
    return
}

$vbs = Join-Path $PSScriptRoot 'copy-content.vbs'
if (-not (Test-Path -LiteralPath $vbs)) { throw "스크립트가 없습니다: $vbs" }

$sc = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
$sc.TargetPath = "$env:WINDIR\System32\wscript.exe"
$sc.Arguments = "//B //Nologo `"$vbs`""   # SendTo 가 뒤에 파일 경로를 덧붙인다
$sc.IconLocation = "$env:WINDIR\System32\imageres.dll,-5302"
$sc.Save()
Write-Host "등록: $lnk"
