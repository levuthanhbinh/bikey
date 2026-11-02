<#
.SYNOPSIS
  Adds bikey to this user's keyboard list and makes it the default input method,
  or with -Remove takes it out again. Other languages and keyboards are kept.

  These are per-user settings, so this must run as the signed-in user. New
  languages go last, so the Windows display language does not change.
#>
param([switch]$Remove)

$ErrorActionPreference = 'Stop'
$tip = '042A:{645A0A82-CCAE-4C5E-BECB-6B1946EDE140}{F72C29DE-6190-4AF5-B5FD-2B2A52F997F2}'

$list = Get-WinUserLanguageList
$vietnamese = $list | Where-Object { $_.LanguageTag -like 'vi*' } | Select-Object -First 1

if ($Remove) {
    if (-not $vietnamese) {
        $bikeyOnly = $false
    } else {
        [void]$vietnamese.InputMethodTips.Remove($tip)
        $bikeyOnly = $vietnamese.InputMethodTips.Count -eq 0
        if ($bikeyOnly) { [void]$list.Remove($vietnamese) }
        Set-WinUserLanguageList -LanguageList $list -Force
    }
    $current = Get-WinDefaultInputMethodOverride
    if ($current -and $current.InputMethodTip -eq $tip) {
        Set-WinDefaultInputMethodOverride -InputTip ''
    }
    return
}

if (-not $vietnamese) {
    $list.Add('vi-VN')
    $vietnamese = $list | Where-Object { $_.LanguageTag -like 'vi*' } | Select-Object -First 1
    # Adding the language brings Windows' own Vietnamese keyboard, which nobody asked for.
    $vietnamese.InputMethodTips.Clear()
}
if ($vietnamese.InputMethodTips -notcontains $tip) {
    $vietnamese.InputMethodTips.Add($tip)
}
Set-WinUserLanguageList -LanguageList $list -Force
Set-WinDefaultInputMethodOverride -InputTip $tip
