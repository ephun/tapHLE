; This Source Code Form is subject to the terms of the Mozilla Public
; License, v. 2.0. If a copy of the MPL was not distributed with this
; file, You can obtain one at https://mozilla.org/MPL/2.0/.
;
; Windows installer for tapHLE, built with Inno Setup 6.
;
; Build it from a finished bundle directory:
;
;   cd dev-scripts
;   ./make-windows-bundle.sh ../target/release/tapHLE.exe
;   iscc /DBundleDir=..\tapHLE_windows_bundle /DAppVersion=0.3.0-alpha.1 tapHLE.iss
;
; It installs per user, into %LOCALAPPDATA%\Programs\tapHLE, and asks for no
; administrator rights. That is not laziness: tapHLE keeps its saved app data,
; its library and its options files beside its executables, and a program in
; Program Files cannot write to its own directory. Installing per user keeps
; the portable layout intact, so an installed copy and an unpacked one behave
; identically and either can be moved to the other.
;
; Uninstalling removes what was installed and nothing else. tapHLE_apps,
; tapHLE_sandbox and tapHLE_frontend are the person's own files — their apps,
; their saved games, their library — and Inno Setup does not touch files it
; did not put there.

#ifndef BundleDir
  #define BundleDir "..\tapHLE_windows_bundle"
#endif
#ifndef AppVersion
  #define AppVersion "0.0.0-unknown"
#endif

[Setup]
; Fixed for the life of the project: it is how Windows recognises an upgrade
; of an existing installation rather than a second copy.
AppId={{4F1B4E0C-9C3E-4B3A-9D8E-6E1B2F7A5C10}
AppName=tapHLE
AppVersion={#AppVersion}
AppVerName=tapHLE {#AppVersion}
AppPublisher=tapHLE project contributors
AppPublisherURL=https://github.com/ephun/tapHLE
AppSupportURL=https://github.com/ephun/tapHLE/issues
AppUpdatesURL=https://github.com/ephun/tapHLE/releases
DefaultDirName={autopf}\tapHLE
DefaultGroupName=tapHLE
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputBaseFilename=tapHLE-{#AppVersion}-windows-x86_64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\res\icon.ico
UninstallDisplayIcon={app}\tapHLE-gui.exe
UninstallDisplayName=tapHLE
LicenseFile={#BundleDir}\COPYING.txt
; The emulator is experimental and its compatibility is per app; saying so
; before installation is more useful than saying it afterwards.
InfoBeforeFile=
AllowNoIcons=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; \
  GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
; The frontend, and the emulator it launches. Both are installed: the
; emulator is a usable program on its own and the command line is not a
; second-class way to run tapHLE.
Source: "{#BundleDir}\tapHLE-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\tapHLE.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\tapHLE_dylibs\*"; DestDir: "{app}\tapHLE_dylibs"; \
  Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#BundleDir}\tapHLE_fonts\*"; DestDir: "{app}\tapHLE_fonts"; \
  Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#BundleDir}\tapHLE_default_options.txt"; DestDir: "{app}"; Flags: ignoreversion
; The user's own options file is only placed if there is not one already, so
; an upgrade never discards what somebody put in it.
Source: "{#BundleDir}\tapHLE_options.txt"; DestDir: "{app}"; Flags: onlyifdoesntexist
Source: "{#BundleDir}\OPTIONS_HELP.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BundleDir}\COPYING.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\res\icon.png"; DestDir: "{app}\res"; Flags: ignoreversion
Source: "{#BundleDir}\tapHLE_apps\README.txt"; DestDir: "{app}\tapHLE_apps"; \
  Flags: onlyifdoesntexist

[Dirs]
; Created up front so that the first run has somewhere to put an app and its
; saved data without having to create it.
Name: "{app}\tapHLE_apps"
Name: "{app}\tapHLE_sandbox"

[Icons]
Name: "{group}\tapHLE"; Filename: "{app}\tapHLE-gui.exe"; \
  WorkingDir: "{app}"; Comment: "Run early iPhone OS apps"
Name: "{group}\tapHLE apps folder"; Filename: "{app}\tapHLE_apps"
Name: "{autodesktop}\tapHLE"; Filename: "{app}\tapHLE-gui.exe"; \
  WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\tapHLE-gui.exe"; Description: "Start tapHLE"; \
  WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
