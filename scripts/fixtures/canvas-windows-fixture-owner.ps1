param([Parameter(Mandatory=$true)][string]$Root,[Parameter(Mandatory=$true)][string]$Report)
$ErrorActionPreference='Stop'
$target=(Resolve-Path -LiteralPath $Root).Path.TrimEnd('\')
$item=Get-Item -LiteralPath $target -Force
if ($item.Name -notmatch '^rc22-native-notifications-r[1-9][0-9]*$' -or $item.Parent.Name -ne 'rovai-canvas-native-190cf86d' -or $item.Parent.Parent.Name -ne 'Temp') { throw 'Only the exact owned notification fixture root is admitted' }
$core=(Resolve-Path -LiteralPath (Join-Path $target 'Core')).Path
$user=[Security.Principal.WindowsIdentity]::GetCurrent().User
$admin=New-Object Security.Principal.SecurityIdentifier('S-1-5-32-544')
$candidates=@(foreach ($name in @('skills','runtime-private','quick-chat')) {
 $path=Join-Path $core $name
 if (Test-Path -LiteralPath $path) { Get-Item -LiteralPath $path -Force; Get-ChildItem -LiteralPath $path -Force -Recurse }
})
if ($candidates | Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or -not $_.FullName.StartsWith($core+'\',[StringComparison]::OrdinalIgnoreCase) }) { throw 'Fixture traversal encountered an unowned or reparse path' }
$before=@($candidates | ForEach-Object {
 $acl=Get-Acl -LiteralPath $_.FullName
 if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -eq $admin.Value) {
  [ordered]@{path=$_.FullName;directory=$_.PSIsContainer;sddl=$acl.Sddl;sha256=if ($_.PSIsContainer) {$null} else {(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}}
 }
})
$snapshot=[ordered]@{scope='Owned generated test data only; elevated token default owner compatibility remains a production R09 diagnostic';root=$target;current_user_sid=$user.Value;before=$before;after=@()}
$bytes=[Text.Encoding]::UTF8.GetBytes(($snapshot | ConvertTo-Json -Depth 6))
$stream=[IO.File]::Open($Report,[IO.FileMode]::CreateNew);try {$stream.Write($bytes,0,$bytes.Length)} finally {$stream.Dispose()}
foreach ($entry in $before) {
 $acl=Get-Acl -LiteralPath $entry.path
 if ($acl.Sddl -ne $entry.sddl) { throw 'Fixture permissions changed after snapshot' }
 $originalAccess=$acl.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access)
 $acl.SetOwner($user);Set-Acl -LiteralPath $entry.path -AclObject $acl
 $after=Get-Acl -LiteralPath $entry.path
 if ($after.GetOwner([Security.Principal.SecurityIdentifier]).Value -ne $user.Value) { throw 'Fixture owner preparation did not apply' }
 if ($after.GetSecurityDescriptorSddlForm([Security.AccessControl.AccessControlSections]::Access) -ne $originalAccess) { throw 'Fixture DACL changed during owner preparation' }
 if (-not $entry.directory -and (Get-FileHash -LiteralPath $entry.path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $entry.sha256) { throw 'Fixture bytes changed during owner preparation' }
 $snapshot.after+=@([ordered]@{path=$entry.path;sddl=$after.Sddl;content_unchanged=$true})
}
[IO.File]::WriteAllText($Report,($snapshot | ConvertTo-Json -Depth 6),(New-Object Text.UTF8Encoding($false)))
Write-Output ('Prepared owned fixture objects: '+$before.Count)
