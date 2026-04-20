; RustStudy NSIS installer hooks.
;
; Chooses a default install directory at startup:
;   - D:\RustStudy if the D: drive exists (preferred — keeps C: clean)
;   - C:\RustStudy otherwise
;
; The user can still change the path in the installer UI; this only sets
; the *default* that the directory page shows.
;
; Tauri's NSIS template defines its own .onInit for per-machine elevation
; and per-user redirection. We override the default InstallDir AFTER it
; runs by hooking into NSIS_HOOK_PREINSTALL — that runs before any file
; copy but after the directory selection page, which means if the user
; accepts the default we steer them to D:\. If they manually picked a
; path, we leave it alone.

!macro NSIS_HOOK_PREINSTALL
  ; Only override when the path still looks like Tauri's default
  ; ($PROGRAMFILES64\RustStudy or $PROGRAMFILES\RustStudy). If the user
  ; picked something custom we respect it.
  StrCmp $INSTDIR "$PROGRAMFILES64\RustStudy" maybe_redirect 0
  StrCmp $INSTDIR "$PROGRAMFILES\RustStudy" maybe_redirect skip_redirect

maybe_redirect:
  ; Does D:\ exist? NSIS checks drive roots via IfFileExists with trailing \
  IfFileExists "D:\*.*" use_d use_c

use_d:
  StrCpy $INSTDIR "D:\RustStudy"
  Goto skip_redirect

use_c:
  StrCpy $INSTDIR "C:\RustStudy"
  Goto skip_redirect

skip_redirect:
!macroend

; No post-install or uninstall actions needed; Tauri handles the rest.
