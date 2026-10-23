param([int]$TestProcessId, [string]$FirstTitle, [string]$SecondTitle, [string]$Report)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Wait-For([scriptblock]$Read) {
    $taskDeadline = [DateTime]::UtcNow.AddSeconds(12)
    do {
        try { $taskPollObservation = & $Read; if ($taskPollObservation) { return $taskPollObservation } }
        catch [System.Windows.Automation.ElementNotAvailableException] {}
        Start-Sleep -Milliseconds 40
    } while ([DateTime]::UtcNow -lt $taskDeadline)
    throw "Timed out waiting for the owned accessibility fixture: $Read"
}
function Find-Name($Parent, [string]$Name) {
    $Parent.FindFirst([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $Name))
}
function Find-Window([string]$Title) {
    $taskCondition = [System.Windows.Automation.AndCondition]::new(
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $TestProcessId),
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::NameProperty, $Title))
    [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $taskCondition)
}
function Assert($Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
try {
    $taskStage = 'initial controls'
    $taskFirst = Wait-For { Find-Window $FirstTitle }
    $taskSecond = Wait-For { Find-Window $SecondTitle }
    $taskIncrement = Wait-For { Find-Name $taskFirst 'Increment' }
    $taskInvoke = $taskIncrement.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
    $taskInvoke.Invoke()
    $null = Wait-For { Find-Name $taskFirst 'Count 1' }
    Assert ($null -ne (Find-Name $taskSecond 'Count 0')) 'An action crossed document boundaries'
    $taskChoice = Wait-For { Find-Name $taskFirst 'Choice' }
    $taskToggle = $taskChoice.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    $taskToggle.Toggle()
    $null = Wait-For { $taskToggle.Current.ToggleState -eq [System.Windows.Automation.ToggleState]::On }
    $taskEditor = Wait-For { Find-Name $taskFirst 'Editor' }
    $null = Wait-For { $taskEditor.SetFocus(); $taskEditor.Current.HasKeyboardFocus }
    $taskValue = $taskEditor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $taskText = 'changed' + [char]::ConvertFromUtf32(0x1F642)
    $taskValue.SetValue($taskText)
    $null = Wait-For { $taskValue.Current.Value -eq $taskText }
    $taskBounds = $taskEditor.Current.BoundingRectangle
    Assert ($taskBounds.Width -gt 0 -and $taskBounds.Height -gt 0) 'Editor has no native bounds'
    $taskDisabled = Wait-For { Find-Name $taskFirst 'Disabled' }
    Assert (-not $taskDisabled.Current.IsEnabled) 'Disabled button was exposed as enabled'
    $taskRefused = $false
    try { $taskDisabled.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke() }
    catch { $taskRefused = $true }
    Assert $taskRefused 'Disabled Invoke was accepted'
    Assert ($null -ne (Find-Name $taskFirst 'Count 1')) 'Disabled Invoke changed the counter'
    $taskSecret = Wait-For { Find-Name $taskFirst 'Secret' }
    Assert $taskSecret.Current.IsPassword 'Password role was lost'
    $taskSecretReadable = $false
    try { $taskSecretReadable = -not [string]::IsNullOrEmpty($taskSecret.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value) }
    catch {}
    Assert (-not $taskSecretReadable) 'Password value crossed the platform bridge'
    $taskRows = Wait-For { Find-Name $taskFirst 'Rows' }
    $taskItems = $taskRows.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem))
    Assert ($taskItems.Count -gt 0 -and $taskItems.Count -lt 50) 'Virtual list expanded its data source'
    $taskStage = 'row editor discovery'
    $taskRowEditor = Wait-For { Find-Name $taskRows 'Row editor 0' }
    $taskStage = 'row focus'
    $null = Wait-For { $taskRowEditor.SetFocus(); $taskRowEditor.Current.HasKeyboardFocus }
    $taskRowValue = $taskRowEditor.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $taskDraft = 'retained' + [char]::ConvertFromUtf32(0x1F642)
    $taskStage = 'row value'
    $taskRowValue.SetValue($taskDraft)
    $null = Wait-For { $taskRowValue.Current.Value -eq $taskDraft }
    $taskRow = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($taskRowEditor)
    Assert ($taskRow.Current.ControlType -eq [System.Windows.Automation.ControlType]::ListItem) 'Row wrapper is not a system ListItem'
    $taskRowIdentity = $taskRow.GetRuntimeId() -join ','
    $null = $taskRow.GetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern)
    $taskStage = 'far seek'
    Assert $taskRowEditor.Current.HasKeyboardFocus 'Editor focus was lost before the far seek'
    (Find-Name $taskFirst 'Arm far scroll').GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    # UIA Invoke first focuses its button. The fixture performs the queued seek
    # when this editor regains focus, exercising a real retained editing row.
    $null = Wait-For { $taskRowEditor.SetFocus(); $taskRowEditor.Current.HasKeyboardFocus }
    $taskFarIndex = Wait-For {
        $taskEdits = $taskRows.FindAll([System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit))
        foreach ($taskEdit in $taskEdits) {
            if ($taskEdit.Current.Name -match '^Row editor (\d+)$' -and [long]$Matches[1] -gt 80000000) { return [long]$Matches[1] }
        }
        return $false
    }
    $taskRowsBounds = $taskRows.Current.BoundingRectangle
    $taskPinnedBounds = $taskRow.Current.BoundingRectangle
    Assert ($taskPinnedBounds.Bottom -le $taskRowsBounds.Top -or $taskPinnedBounds.Top -ge $taskRowsBounds.Bottom) 'The pinned row never left the viewport'
    $taskRowEditor = Wait-For { Find-Name $taskRows 'Row editor 0' }
    $taskRow = [System.Windows.Automation.TreeWalker]::ControlViewWalker.GetParent($taskRowEditor)
    Assert (($taskRow.GetRuntimeId() -join ',') -eq $taskRowIdentity) 'Far scrolling replaced the pinned row identity'
    Assert $taskRowEditor.Current.HasKeyboardFocus 'Far scrolling lost the editor focus'
    $taskScrollItem = $taskRow.GetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern)
    $taskStage = 'row reveal'
    $taskScrollItem.ScrollIntoView()
    $null = Wait-For {
        $taskVisible = $taskRow.Current.BoundingRectangle
        $taskViewport = $taskRows.Current.BoundingRectangle
        $taskVisible.Top -ge ($taskViewport.Top - 1) -and $taskVisible.Top -lt $taskViewport.Bottom
    }
    Assert ($taskRowValue.Current.Value -eq $taskDraft) 'ScrollIntoView reset the editor draft'
    Assert $taskRowEditor.Current.HasKeyboardFocus 'ScrollIntoView moved keyboard focus'
    Assert (($taskRow.GetRuntimeId() -join ',') -eq $taskRowIdentity) 'ScrollIntoView replaced the row identity'
    $taskResult = [ordered]@{ok=$true;processId=$TestProcessId;windows=2;counter=1;isolated=$true;toggle='On';value=$taskText;focused=$true;disabledRefused=$taskRefused;passwordProtected=$true;listItems=$taskItems.Count;farIndex=$taskFarIndex;rowReveal=$true;rowIdentity=$true;rowFocus=$true;rowDraft=$true;bounds=@{width=$taskBounds.Width;height=$taskBounds.Height};runtime=[Environment]::Version.ToString()}
    $taskResult | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $Report -Encoding UTF8
    exit 0
} catch {
    $taskFailure = [ordered]@{ok=$false;stage=$taskStage;line=$_.InvocationInfo.ScriptLineNumber;error=$_.Exception.ToString();processId=$TestProcessId}
    # Keep focus failures diagnosable without recording another app's title,
    # text or control names. These reads do not alter focus or weaken assertions.
    try {
        $taskFocused = [System.Windows.Automation.AutomationElement]::FocusedElement
        $taskFailure.focusedProcessIsFixture = $null -ne $taskFocused -and $taskFocused.Current.ProcessId -eq $TestProcessId
        if ($null -ne $taskEditor) {
            $taskFailure.editor = @{focusable=$taskEditor.Current.IsKeyboardFocusable;focused=$taskEditor.Current.HasKeyboardFocus;enabled=$taskEditor.Current.IsEnabled;offscreen=$taskEditor.Current.IsOffscreen}
        }
        if ($null -ne $taskFirst) { $taskFailure.firstHandle = $taskFirst.Current.NativeWindowHandle }
        if ($null -ne $taskSecond) { $taskFailure.secondHandle = $taskSecond.Current.NativeWindowHandle }
    } catch { $taskFailure.diagnosticError = $_.Exception.GetType().FullName }
    $taskFailure | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Report -Encoding UTF8
    exit 1
}
