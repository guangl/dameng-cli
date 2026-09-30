Register-ArgumentCompleter -Native -CommandName dm -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)
    $words = @($commandAst.CommandElements | Select-Object -Skip 1 | Where-Object { $_.Extent.StartOffset -lt $cursorPosition } | ForEach-Object {
        if ($_ -is [System.Management.Automation.Language.StringConstantExpressionAst]) { $_.Value }
        else { $_.Extent.Text }
    })
    if ($wordToComplete -ne '') {
        if ($words.Count -gt 0) { $words[-1] = $wordToComplete }
        else { $words += $wordToComplete }
        $candidates = @(dm complete -- @words 2>$null)
    } else {
        $candidates = @(dm complete --empty-word -- @words 2>$null)
    }
    foreach ($candidate in $candidates) {
        $quoted = "'" + $candidate.Replace("'", "''") + "'"
        [System.Management.Automation.CompletionResult]::new($quoted, $candidate, 'ParameterValue', $candidate)
    }
}
