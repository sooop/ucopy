' 탐색기 보내기(SendTo)용: 인자로 받은 파일의 내용을 ucopy 로 클립보드에 복사한다.
' 콘솔 창 없이 실행되며, 인코딩 판별(UTF-8/UTF-16/ANSI)은 ucopy 가 처리한다.
Option Explicit

Const UCOPY = "C:\apps\bin\clip.exe"
Const MAX_BYTES = 20971520 ' 20MB 초과 파일은 거부

Dim fso, sh, path, f, stream, bytes, i, cmd
Set fso = CreateObject("Scripting.FileSystemObject")
Set sh = CreateObject("WScript.Shell")

If WScript.Arguments.Count = 0 Then WScript.Quit 2
path = WScript.Arguments(0) ' 다중 선택 시 탐색기가 파일마다 따로 실행하므로 한 개만 온다

If Not fso.FileExists(path) Then
  sh.Popup "파일이 아닙니다: " & path, 3, "내용 복사", 48
  WScript.Quit 1
End If

Set f = fso.GetFile(path)
If f.Size > MAX_BYTES Then
  sh.Popup "파일이 너무 큽니다 (" & Int(f.Size / 1048576) & "MB > 20MB)", 3, "내용 복사", 48
  WScript.Quit 1
End If

' 앞쪽 8KB 에 NUL 바이트가 있으면 바이너리로 보고 거부 (UTF-16 BOM 파일은 허용)
Set stream = CreateObject("ADODB.Stream")
stream.Type = 1
stream.Open
stream.LoadFromFile path
If stream.Size >= 2 Then
  bytes = stream.Read(2)
  If Not (AscB(MidB(bytes, 1, 1)) = &HFF And AscB(MidB(bytes, 2, 1)) = &HFE) Then
    stream.Position = 0
    bytes = stream.Read(8192)
    For i = 1 To LenB(bytes)
      If AscB(MidB(bytes, i, 1)) = 0 Then
        stream.Close
        sh.Popup "바이너리 파일은 복사하지 않습니다.", 3, "내용 복사", 48
        WScript.Quit 1
      End If
    Next
  End If
End If
stream.Close

' cmd /c 는 바깥쪽 따옴표를 한 겹 벗기므로 전체를 한 번 더 감싼다. 창 스타일 0 = 숨김
cmd = "cmd /c """"" & UCOPY & """ < """ & path & """"""
If sh.Run(cmd, 0, True) <> 0 Then
  sh.Popup "클립보드 복사에 실패했습니다.", 3, "내용 복사", 16
  WScript.Quit 1
End If
