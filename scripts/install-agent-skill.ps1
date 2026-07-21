[CmdletBinding()]
param([ValidateSet("codex","generic")][string]$Agent="codex",[string]$SkillsDir,[string]$Source,[switch]$Force,[switch]$InstallCli,[switch]$SkipCli)
$scriptDir=Split-Path -Parent $MyInvocation.MyCommand.Path
$arguments=@("-Mode","skills-only","-OnlyScan","-SkipAgent","-Agent",$Agent)
if($SkillsDir){$arguments+=@("-SkillsDir",$SkillsDir)};if($Source){$arguments+=@("-Source",$Source)};if($Force){$arguments+="-Force"};if($SkipCli){$arguments+="-SkipCli"}
& (Join-Path $scriptDir "install-agent-workflow.ps1") @arguments
exit $LASTEXITCODE
