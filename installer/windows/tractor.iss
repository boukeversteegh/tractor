; Tractor Windows Installer - Inno Setup Script
; Builds a Windows installer for the tractor CLI tool

#ifndef VERSION
  #define VERSION "dev"
#endif

[Setup]
AppName=Tractor
AppVersion={#VERSION}
AppPublisher=Tractor Contributors
AppPublisherURL=https://github.com/boukeversteegh/tractor
AppSupportURL=https://github.com/boukeversteegh/tractor/issues
DefaultDirName={autopf}\Tractor
OutputBaseFilename=tractor-{#VERSION}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesEnvironment=yes
MinVersion=10.0
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
SetupIconFile=tractor.ico
UninstallDisplayIcon={app}\tractor.exe
OutputDir=output
WizardStyle=modern

[Files]
Source: "tractor.exe"; DestDir: "{app}"; Flags: ignoreversion

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
var
  Path: string;
  AppDir: string;
begin
  if CurStep = ssPostInstall then
  begin
    AppDir := ExpandConstant('{app}');
    if IsAdminInstallMode then
    begin
      RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
      if Pos(Uppercase(AppDir), Uppercase(Path)) = 0 then
      begin
        Path := Path + ';' + AppDir;
        RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
      end;
    end
    else
    begin
      RegQueryStringValue(HKCU, 'Environment', 'Path', Path);
      if Pos(Uppercase(AppDir), Uppercase(Path)) = 0 then
      begin
        Path := Path + ';' + AppDir;
        RegWriteStringValue(HKCU, 'Environment', 'Path', Path);
      end;
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path: string;
  AppDir: string;
  P: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    AppDir := ExpandConstant('{app}');
    if IsAdminInstallMode then
    begin
      RegQueryStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
      P := Pos(';' + Uppercase(AppDir), Uppercase(Path));
      if P > 0 then
      begin
        Delete(Path, P, Length(AppDir) + 1);
        RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
      end
      else
      begin
        P := Pos(Uppercase(AppDir) + ';', Uppercase(Path));
        if P > 0 then
        begin
          Delete(Path, P, Length(AppDir) + 1);
          RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
        end
        else
        begin
          StringChangeEx(Path, AppDir, '', True);
          RegWriteStringValue(HKLM, 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment', 'Path', Path);
        end;
      end;
    end
    else
    begin
      RegQueryStringValue(HKCU, 'Environment', 'Path', Path);
      P := Pos(';' + Uppercase(AppDir), Uppercase(Path));
      if P > 0 then
      begin
        Delete(Path, P, Length(AppDir) + 1);
        RegWriteStringValue(HKCU, 'Environment', 'Path', Path);
      end
      else
      begin
        P := Pos(Uppercase(AppDir) + ';', Uppercase(Path));
        if P > 0 then
        begin
          Delete(Path, P, Length(AppDir) + 1);
          RegWriteStringValue(HKCU, 'Environment', 'Path', Path);
        end
        else
        begin
          StringChangeEx(Path, AppDir, '', True);
          RegWriteStringValue(HKCU, 'Environment', 'Path', Path);
        end;
      end;
    end;
  end;
end;
