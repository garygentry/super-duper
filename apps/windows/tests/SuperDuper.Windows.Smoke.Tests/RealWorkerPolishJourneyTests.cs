using System.Diagnostics;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Text.Json;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;
using SuperDuper.Windows.Services;

namespace SuperDuper.Windows.Smoke.Tests;

/// <summary>Opt-in, production WPF + real worker evidence. Run through Invoke-WindowsPolishJourney.ps1
/// in its own test host: Application lifetime must not be shared with the fictional smoke fixture.</summary>
[TestClass]
[DoNotParallelize]
public sealed class RealWorkerPolishJourneyTests
{
    public TestContext TestContext { get; set; } = null!;
    private readonly List<object> _steps = [];
    private string _state = string.Empty;
    private readonly CancellationTokenSource _deadline = new(TimeSpan.FromMinutes(4));

    [TestMethod]
    [TestCategory("RealWorkerPolish")]
    public void ProductionWpf_RealFiles_ScanPageDecideCheckAndRestart()
    {
        var corpus = Environment.GetEnvironmentVariable("SUPER_DUPER_POLISH_CORPUS");
        var state = Environment.GetEnvironmentVariable("SUPER_DUPER_POLISH_STATE");
        var worker = Environment.GetEnvironmentVariable("SUPER_DUPER_POLISH_WORKER");
        if (string.IsNullOrEmpty(corpus) || string.IsNullOrEmpty(state) || string.IsNullOrEmpty(worker))
            Assert.Inconclusive("Opt-in real-file journey: use scripts/Invoke-WindowsPolishJourney.ps1.");
        _state = state!;
        Assert.IsTrue(File.Exists(Path.Combine(corpus!, "source-manifest.json")));
        Assert.IsTrue(File.Exists(worker));
        Assert.IsFalse(File.Exists(Path.Combine(_state, "super_duper.db")), "Each journey requires fresh private state.");
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            var dispatcher = Dispatcher.CurrentDispatcher;
            SynchronizationContext.SetSynchronizationContext(new DispatcherSynchronizationContext(dispatcher));
            dispatcher.BeginInvoke(new Action(async () =>
            {
                try { await RunJourneyAsync(corpus!, worker!); }
                catch (Exception exception) { failure = exception; Record("failure", exception.ToString()); }
                finally { dispatcher.BeginInvokeShutdown(DispatcherPriority.Send); }
            }));
            Dispatcher.Run();
        }) { IsBackground = true, Name = "Real-worker production WPF journey" };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.IsTrue(thread.Join(TimeSpan.FromMinutes(5)), "Production WPF journey exceeded its bounded deadline.");
        TestContext.WriteLine($"Real-worker WPF evidence: {_state}");
        if (failure is not null) ExceptionDispatchInfo.Capture(failure).Throw();
    }

    private async Task RunJourneyAsync(string corpus, string executable)
    {
        // No production startup is called: App provides shipping resources, while every worker path is private.
        var app = new Application { ShutdownMode = ShutdownMode.OnExplicitShutdown, ThemeMode = ThemeMode.System };
        app.Resources.MergedDictionaries.Add(new ResourceDictionary
        {
            Source = new Uri("/SuperDuper.Windows;component/Resources/ShellResources.xaml", UriKind.Relative),
        });
        app.Resources.Add("BooleanToVisibilityConverter", new BooleanToVisibilityConverter());
        long runId = 0;
        long reviewRevision = 0;
        var preferences = new JourneyPresentationStore(_state);
        for (var launch = 0; launch < 2; launch++)
        {
            // Reuse the existing test-only isolation constructor without adding a shipping automation API.
            using var worker = (WorkerClient)Activator.CreateInstance(typeof(WorkerClient),
                BindingFlags.Instance | BindingFlags.NonPublic, null,
                [executable, TimeSpan.FromSeconds(15), Path.Combine(_state, "super_duper.db"),
                    Path.Combine(_state, "worker.log"), Path.Combine(_state, "hash-cache")], null)!;
            using var shell = new ShellViewModel(worker, new FolderPickerService(), new ReadOnlyCheckConfirmation(),
                new WpfUiDispatcher(Dispatcher.CurrentDispatcher), new WpfClipboardService(),
                new WindowsExplorerService(), new WindowsCloudLocationService());
            var window = new MainWindow(shell, worker, ownsWorkerLifetime: false, preferencesStore: preferences)
            {
                Width = 1180, Height = 760, ShowActivated = false, ShowInTaskbar = false,
                WindowStartupLocation = WindowStartupLocation.Manual, Left = -10000, Top = -10000,
            };
            // WPF bitmap rendering omits the native window background; match the live Fluent brush.
            window.SetResourceReference(Window.BackgroundProperty, "ApplicationBackgroundBrush");
            int? ownedPid = null;
            try
            {
                window.Show();
                await Await(window.InitializeAsync());
                Assert.IsTrue(shell.IsConnected, shell.StatusDetail);
                ownedPid = (int?)typeof(WorkerClient).GetProperty("OwnedProcessId", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(worker);
                Record($"launch-{launch + 1}", new { Worker = executable, WorkerSha256 = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(executable))), UiAssemblySha256 = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(typeof(MainWindow).Assembly.Location))), TestHost = Environment.ProcessId, OwnedWorkerPid = typeof(WorkerClient).GetProperty("OwnedProcessId", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(worker), Corpus = corpus, State = _state });
                if (launch == 1)
                {
                    Assert.AreEqual(1, shell.Sessions.Items.Count);
                    Assert.IsTrue(window.IsSavedScanPaneOpen, "Remembered selector should reopen at wide width after restart.");
                    var restoredHistoryDetails = (Expander)((FrameworkElement)window.FindName("HistoryWorkspace")).FindName("WarningTechnicalDetails");
                    Assert.IsTrue(restoredHistoryDetails.IsExpanded, "A disclosure in an inactive tab must restore too.");
                    Assert.AreEqual(WorkspaceDestination.FolderResults, shell.ResultsDestination, "Restart should retain the last Files/Folders choice.");
                    shell.SelectedDestination = WorkspaceDestination.ScanSetup;
                    await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
                    Assert.IsTrue(Find<Expander>(window, "SetupAdvanced").IsExpanded, "Named advanced section should restore its disclosure choice.");
                    Record("preferences-restored", preferences.Value);
                    shell.Sessions.SelectedSession = shell.Sessions.Items.Single();
                    await Until(() => !shell.IsLoadingSession && shell.History.Runs.Count == 1, "restored history");
                    shell.SelectedDestination = WorkspaceDestination.History;
                    shell.History.SelectedRun = shell.History.Runs.Single();
                    await CaptureAsync(window, "08-restarted-history");
                    shell.OpenScanCommand.Execute(null);
                    await Until(() => shell.SelectedRun?.Id == runId, "reopened original run");
                    var persisted = await Await(worker.GetReviewPlanAsync(runId, _deadline.Token));
                    Assert.AreEqual(reviewRevision, persisted.Plan.Revision);
                    Assert.AreEqual(1L, persisted.Summary.RemoveCount);
                    Assert.AreEqual(1L, persisted.Summary.FolderRemoveCount);
                    Assert.AreEqual("completed", (await Await(worker.GetRunAsync(runId, _deadline.Token))).Status);
                    await Until(() => !shell.DuplicateFiles.IsLoading && shell.DuplicateFiles.Groups.Count > 0, "restored file results");
                    await CaptureAsync(window, "09-restarted-results");
                    Record("restart-persisted", persisted);
                    await ExerciseMutationAsync(window, shell, worker, corpus);
                    var baselineRun = await Await(worker.GetRunAsync(runId, _deadline.Token));
                    await ExerciseCancelAndRepeatAsync(window, shell, worker, baselineRun.SessionId);
                    continue;
                }

                Assert.AreEqual(0, shell.Sessions.Items.Count);
                await CaptureAsync(window, "01-empty");
                await Await(shell.Sessions.NewSessionCommand.ExecuteAsync(null));
                await Until(() => !shell.Setup.IsDetectingCloudLocations, "cloud location detection");
                Find<TextBox>(window, "SessionName").Text = "Real-file polish journey";
                var advanced = Find<Expander>(window, "SetupAdvanced");
                Assert.IsFalse(advanced.IsExpanded);
                shell.Setup.Roots[0].Path = "not-an-absolute-path";
                await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
                Assert.IsTrue(shell.Setup.HasValidationErrors);
                var validation = Descendants<TextBlock>(window).First(text => text.Text == shell.Setup.ValidationMessage && text.IsVisible && !HasAncestor(text, advanced));
                Assert.IsTrue(validation.IsVisible, "Validation must remain available outside collapsed advanced settings.");
                Assert.IsFalse(HasAncestor(validation, advanced));
                validation.BringIntoView();
                await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
                var validationBounds = validation.TransformToAncestor(window).TransformBounds(new Rect(validation.RenderSize));
                Assert.IsTrue(validationBounds.Bottom > 0 && validationBounds.Top < window.ActualHeight, "Validation must be reachable by the page scroll without opening advanced settings.");
                await CaptureAsync(window, "02-invalid-location-advanced-closed");
                shell.Setup.Roots[0].Path = Path.Combine(corpus, "Working library");
                shell.Setup.AddRootCommand.Execute(null);
                shell.Setup.Roots[^1].Path = Path.Combine(corpus, "Backup archive");
                Assert.IsTrue(shell.CanStartRun, shell.Setup.ValidationMessage + shell.Setup.CloudDetectionMessage);
                Find<TextBox>(window, "SessionName").BringIntoView();
                await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
                await CaptureAsync(window, "02-setup");
                advanced.IsExpanded = true;
                var hiddenHistoryDetails = (Expander)((FrameworkElement)window.FindName("HistoryWorkspace")).FindName("WarningTechnicalDetails");
                hiddenHistoryDetails.IsExpanded = true;
                
                window.IsSavedScanPaneOpen = true;
                await Until(() => preferences.Value.IsSectionExpanded("SetupAdvanced") && preferences.Value.IsSectionExpanded("WarningTechnicalDetails") && preferences.Value.IsSavedScanSelectorExpanded, "persisted active/inactive disclosure and selector choices");
                await Await(shell.StartRunCommand.ExecuteAsync(null));
                await CaptureAsync(window, "03-scan");
                await Until(() => !shell.HasActiveRun && shell.SelectedRun?.Status == "completed", "completed real scan");
                runId = shell.SelectedRun!.Id;
                Record("completed-scan", shell.SelectedRun);
                Assert.IsTrue(shell.SelectedRun.DuplicateFileGroups > 200);
                shell.SelectedDestination = WorkspaceDestination.FileResults;
                await Until(() => !shell.DuplicateFiles.IsLoading && shell.DuplicateFiles.Groups.Count > 0, "file groups");
                var files = shell.DuplicateFiles;
                Assert.IsTrue(files.TotalGroups > 200);
                var firstId = files.Groups[0].Id;
                await Await(files.NextPageCommand.ExecuteAsync(null));
                Assert.AreNotEqual(firstId, files.Groups[0].Id);
                await Await(files.PreviousPageCommand.ExecuteAsync(null));
                Assert.AreEqual(firstId, files.Groups[0].Id);
                files.SearchText = "Project overview";
                await Await(files.ApplyFiltersCommand.ExecuteAsync(null));
                Assert.AreEqual(1L, files.TotalGroups);
                Find<DataGrid>(window, "FileGroupsGrid").SelectedItem = files.Groups.Single();
                await Until(() => !files.IsDetailLoading && files.Members.Count > 0, "many-copy comparison");
                Assert.IsTrue(files.TotalMembers > 200);
                Assert.IsTrue(files.CanMoveMembersNext);
                var firstMember = files.Members[0].Id;
                await Await(files.NextMemberPageCommand.ExecuteAsync(null));
                Assert.AreNotEqual(firstMember, files.Members[0].Id);
                await Await(files.PreviousMemberPageCommand.ExecuteAsync(null));
                await Await(files.KeepMemberCommand.ExecuteAsync(files.Members[0]));
                await Await(files.RemoveMemberCommand.ExecuteAsync(files.Members[1]));
                Assert.IsFalse(files.HasDetailError, files.DetailErrorMessage);
                await ShowComparisonAsync(window, "File", () => files.SelectedMember = files.Members.Single(member => member.Member.Decision == "remove"));
                await CaptureAsync(window, "04-file-decisions");
                window.Height = 720;
                await ShowComparisonAsync(window, "File", () => files.SelectedMember = files.Members.Single(member => member.Member.Decision == "remove"));
                await CaptureAsync(window, "04-file-decisions-720");
                window.Height = 760;
                window.Width = 900; window.Height = 600;
                await ShowComparisonAsync(window, "File", () => files.SelectedMember = files.Members.Single(member => member.Member.Decision == "remove"));
                await CaptureAsync(window, "04-file-decisions-narrow");
                Assert.IsFalse(window.IsSavedScanPaneOpen, "Compact layout must close the selector.");
                Assert.IsTrue(preferences.Value.IsSavedScanSelectorExpanded, "Responsive collapse must not overwrite the remembered wide preference.");
                window.Width = 1180; window.Height = 760;
                window.UpdateLayout();
                await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
                Assert.IsTrue(window.IsSavedScanPaneOpen, "Returning to wide layout should restore the remembered selector.");
                await Await(files.ClearFiltersCommand.ExecuteAsync(null));

                shell.SelectedDestination = WorkspaceDestination.FolderResults;
                await Until(() => !shell.DuplicateFolders.IsLoading && shell.DuplicateFolders.Groups.Count > 0, "real exact-folder sets");
                var folders = shell.DuplicateFolders;
                Find<DataGrid>(window, "FolderGroupsGrid").SelectedItem = folders.Groups[0];
                await Until(() => !folders.IsDetailLoading && folders.Members.Count >= 2, "folder comparison");
                await Await(folders.KeepFolderCommand.ExecuteAsync(folders.Members[0]));
                await Await(folders.RemoveFolderCommand.ExecuteAsync(folders.Members[1]));
                Assert.IsFalse(folders.HasDetailError, folders.DetailErrorMessage);
                await ShowComparisonAsync(window, "Folder", () => folders.SelectedMember = folders.Members.Single(member => member.Member.Decision == "remove"));
                await CaptureAsync(window, "05-folder-decisions");
                window.Width = 900; window.Height = 600;
                await ShowComparisonAsync(window, "Folder", () => folders.SelectedMember = folders.Members.Single(member => member.Member.Decision == "remove"));
                await CaptureAsync(window, "05-folder-decisions-narrow");
                window.Width = 1180; window.Height = 720;
                await ShowComparisonAsync(window, "Folder", () => folders.SelectedMember = folders.Members.Single(member => member.Member.Decision == "remove"));
                await CaptureAsync(window, "05-folder-decisions-720");
                window.Height = 760;
                var review = await Await(worker.GetReviewPlanAsync(runId, _deadline.Token));
                Assert.AreEqual(1L, review.Summary.RemoveCount);
                Assert.AreEqual(1L, review.Summary.FolderRemoveCount);
                Assert.IsTrue(review.Summary.EffectiveRemovalFileCount > 1);
                reviewRevision = review.Plan.Revision;
                Record("worker-review", review);

                shell.SelectedDestination = WorkspaceDestination.History;
                shell.SelectedArea = WorkspaceArea.Results;
                Assert.AreEqual(WorkspaceDestination.FolderResults, shell.SelectedDestination, "Leaving Results must not reset the Files/Folders choice.");
                await Until(() => preferences.Value.LastResultsMode == ResultsDisplayMode.Folders, "persisted results mode");
                shell.SelectedDestination = WorkspaceDestination.Review;
                await Until(() => !shell.Preflight.IsLoading && shell.Preflight.HasReviewRemovals, "review summary");
                await CaptureAsync(window, "06-review-before-check");
                Assert.IsTrue(shell.Preflight.StartCommand.CanExecute(null), shell.Preflight.ErrorMessage);
                await Await(shell.Preflight.StartCommand.ExecuteAsync(null));
                await Until(() => shell.Preflight.IsTerminal, "whole-plan preflight");
                Assert.AreEqual("completed", shell.Preflight.Preflight!.Status, shell.Preflight.ErrorMessage);
                Assert.IsTrue(shell.Preflight.Preflight.ReadyCount > 0);
                Assert.AreEqual(0L, shell.Preflight.Preflight.MissingCount);
                Assert.AreEqual(0L, shell.Preflight.Preflight.ChangedCount);
                await CaptureAsync(window, "07-review-checked");
                Record("preflight", shell.Preflight.Preflight);
                shell.SelectedDestination = WorkspaceDestination.History;
                await Until(() => shell.History.Runs.Count == 1, "history");
                await CaptureAsync(window, "07-history");
                // Every manifested source copy must still exist and retain its original content.
                using var manifest = JsonDocument.Parse(File.ReadAllText(Path.Combine(corpus, "source-manifest.json")));
                foreach (var entry in manifest.RootElement.EnumerateArray())
                {
                    var path = Path.Combine(corpus, entry.GetProperty("Copy").GetString()!);
                    Assert.IsTrue(File.Exists(path), $"Journey removed {path}");
                    Assert.AreEqual(entry.GetProperty("SHA256").GetString(), Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(path))));
                }
                Record("copies-unchanged", manifest.RootElement.GetArrayLength());
            }
            finally
            {
                await Await((Task)typeof(MainWindow).GetField("_preferenceWrite", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(window)!);
                window.Close();
                shell.Dispose();
                await worker.DisposeAsync();
                if (ownedPid is int pid) Assert.IsFalse(Process.GetProcessesByName("super-duper-worker").Any(process => process.Id == pid), $"Owned worker {pid} leaked after disposal.");
                Record($"launch-{launch + 1}-closed", "Window closed; owned worker disposed; stderr drained by production WorkerClient.");
            }
        }
        app.Shutdown();
        Record("passed", "Real-worker loaded-STA production WPF; native keyboard/mouse/dialogs, removal, media preview and drive performance not exercised.");
    }

    private static async Task ShowComparisonAsync(Window window, string prefix, Action selectMarkedCopy)
    {
        window.UpdateLayout();
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        var compare = Find<Button>(window, prefix + "CompareSelectedSet");
        if (compare.IsVisible)
        {
            Assert.IsTrue(compare.IsEnabled);
            compare.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        }
        selectMarkedCopy();
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        window.UpdateLayout();
    }
    private async Task ExerciseCancelAndRepeatAsync(MainWindow window, ShellViewModel shell, WorkerClient worker, long sessionId)
    {
        shell.Sessions.SelectedSession = shell.Sessions.Items.Single(item => item.Id == sessionId);
        await Until(() => !shell.IsLoadingSession && shell.Setup.SessionId == sessionId && !shell.Setup.IsDetectingCloudLocations, "saved large corpus setup");
        shell.SelectedDestination = WorkspaceDestination.ScanSetup;
        shell.Setup.RepeatCachePolicy = RepeatCachePolicyNames.RevalidateContent;
        await Until(() => shell.CanStartRun, "saved scan and history ready for repeat");
        Assert.IsTrue(shell.CanStartRun, shell.Setup.ValidationMessage);
        await Await(shell.StartRunCommand.ExecuteAsync(null));
        await Until(() => shell.Progress.CanCancel || !shell.HasActiveRun, "cancellable real scan");
        Assert.IsTrue(shell.Progress.CanCancel, "The actual-file corpus finished before Stop could be exercised; enlarge the corpus for this environment.");
        var cancelledId = shell.Progress.Run!.Id;
        var cancellation = shell.Progress.CancelCommand.ExecuteAsync(null);
        await CaptureAsync(window, "16-stop-requested");
        await Await(cancellation);
        await Until(() => !shell.HasActiveRun && shell.Progress.Run?.Status == "cancelled", "cancelled real scan");
        Assert.AreEqual("cancelled", (await Await(worker.GetRunAsync(cancelledId, _deadline.Token))).Status);
        Record("cancelled-run", shell.Progress.Run!);
        await CaptureAsync(window, "17-cancelled-scan");
        shell.SelectedDestination = WorkspaceDestination.ScanSetup;
        await Until(() => shell.CanStartRun, "rescan after Stop");
        await Await(shell.StartRunCommand.ExecuteAsync(null));
        await Until(() => !shell.HasActiveRun && shell.SelectedRun is { Status: "completed" } run && run.Id != cancelledId, "rescan after cancellation");
        Assert.IsTrue(shell.SelectedRun!.DuplicateFileGroups > 200);
        Record("rescan-after-cancel", shell.SelectedRun);
        await CaptureAsync(window, "18-rescan-after-cancel");
    }
    private async Task ExerciseRuleAsync(MainWindow window, ShellViewModel shell, WorkerClient worker, WorkerRun run, string preferredRoot, string backupRoot)
    {
        var rules = shell.DuplicateFiles.PreferenceRules;
        await Await(rules.EnsureRunAsync(run, _deadline.Token));
        await Until(() => !rules.IsBusy, "preference rule list ready");
        Assert.IsTrue(rules.NewRuleCommand.CanExecute(null));
        rules.NewRuleCommand.Execute(null);
        rules.RuleName = "Prefer current disposable documents";
        // New rules already contain canonical run roots; choose by path, not by assumed discovery order.
        rules.SelectedRoot = rules.OrderedRoots.Single(root => root.EndsWith("Current documents", StringComparison.Ordinal));
        rules.RemoveRootCommand.Execute(null);
        rules.NewRoot = preferredRoot;
        Assert.IsTrue(rules.AddRootCommand.CanExecute(null), "The visible Add root action must be enabled before invocation.");
        rules.AddRootCommand.Execute(null);
        Assert.AreEqual(2, rules.OrderedRoots.Count);
        rules.SelectedRoot = rules.OrderedRoots.Single(root => root.EndsWith("Current documents", StringComparison.Ordinal));
        rules.MoveRootUpCommand.Execute(null);
        StringAssert.Contains(rules.OrderedRoots[0], "Current documents");
        rules.MoveRootDownCommand.Execute(null);
        StringAssert.Contains(rules.OrderedRoots[0], "Backup documents");
        rules.MoveRootUpCommand.Execute(null);
        StringAssert.Contains(rules.OrderedRoots[0], "Current documents");
        await Await(rules.SaveCommand.ExecuteAsync(null));
        Assert.IsFalse(rules.HasError, rules.ErrorMessage);
        rules.SelectedScope = rules.ScopeOptions.Single(option => option.Kind == PreferencePreviewScopeKind.SelectedSets);
        await Await(rules.PreviewCommand.ExecuteAsync(null));
        Assert.IsFalse(rules.HasError, rules.ErrorMessage);
        Assert.AreEqual(1, rules.PreviewGroups.Count);
        Assert.AreEqual(preferredRoot, rules.PreviewGroups[0].Group.PreferredRoot,
            "The manually entered ordinary root must outrank the canonical backup root.");
        Assert.AreEqual(0L, rules.PreviewGroups[0].Group.BestRank);
        Assert.AreEqual(1L, rules.PreviewGroups[0].Group.ProposedRemovePathCount);
        var previewPlan = await Await(worker.GetReviewPlanAsync(run.Id, _deadline.Token));
        Assert.AreEqual(0L, previewPlan.Summary.RemoveCount, "A rule preview must not record removal decisions.");
        shell.SelectedDestination = WorkspaceDestination.Review;
        await Until(() => !shell.Preflight.IsLoading, "rule review surface");
        Find<Expander>(window, "LocationPreferencesExpander").IsExpanded = true;
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        var previewStage = Find<Expander>(window, "PreferencePreviewStage");
        previewStage.IsExpanded = true; previewStage.BringIntoView();
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        await CaptureAsync(window, "10-rule-preview");
        rules.ApplyCommand.Execute(null);
        Assert.IsTrue(rules.IsApplicationConfirmationVisible);
        await AssertConfirmationEscapeAsync(window, "PreferenceConfirmApplication", "PreferenceApplyRule");
        Assert.IsFalse(rules.IsApplicationConfirmationVisible);
        var cancelledApply = await Await(worker.GetReviewPlanAsync(run.Id, _deadline.Token));
        Assert.AreEqual(previewPlan.Summary.RemoveCount, cancelledApply.Summary.RemoveCount,
            "Escape from Apply must preserve the read-only preview without recording decisions.");
        Assert.IsNull(rules.LatestApplication);
        Record("rule-apply-escape", cancelledApply.Summary);
        rules.ApplyCommand.Execute(null);
        Assert.IsTrue(rules.IsApplicationConfirmationVisible);
        await Await(rules.ConfirmApplicationCommand.ExecuteAsync(null));
        Assert.IsFalse(rules.HasError, rules.ErrorMessage);
        Assert.AreEqual("active", rules.LatestApplication?.State);
        var applicationId = rules.LatestApplication!.Id;
        var applied = await Await(worker.GetReviewPlanAsync(run.Id, _deadline.Token));
        Assert.AreEqual(1L, applied.Summary.RuleRemoveCount);
        Assert.AreEqual(1L, applied.Summary.RuleKeepCount);
        Record("rule-applied", rules.LatestApplication);
        await CaptureAsync(window, "10-rule-applied");
        rules.ReverseCommand.Execute(null);
        Assert.IsTrue(rules.IsReversalConfirmationVisible);
        await AssertConfirmationEscapeAsync(window, "PreferenceConfirmReversal", "PreferenceReverseApplication");
        Assert.IsFalse(rules.IsReversalConfirmationVisible);
        var cancelledReverse = await Await(worker.GetReviewPlanAsync(run.Id, _deadline.Token));
        Assert.AreEqual(applied.Summary.RuleRemoveCount, cancelledReverse.Summary.RuleRemoveCount);
        Assert.AreEqual(applied.Summary.RuleKeepCount, cancelledReverse.Summary.RuleKeepCount);
        Assert.AreEqual(applicationId, rules.LatestApplication!.Id);
        Assert.AreEqual("active", rules.LatestApplication.State,
            "Escape from Reverse must preserve the same active rule application and its decisions.");
        Record("rule-reverse-escape", cancelledReverse.Summary);
        rules.ReverseCommand.Execute(null);
        Assert.IsTrue(rules.IsReversalConfirmationVisible);
        await Await(rules.ConfirmReversalCommand.ExecuteAsync(null));
        Assert.IsFalse(rules.HasError, rules.ErrorMessage);
        Assert.AreEqual(applicationId, rules.LatestApplication!.Id);
        Assert.AreEqual("reversed", rules.LatestApplication.State);
        var reversed = await Await(worker.GetReviewPlanAsync(run.Id, _deadline.Token));
        Assert.AreEqual(0L, reversed.Summary.RuleRemoveCount);
        Assert.AreEqual(0L, reversed.Summary.RuleKeepCount);
        Record("rule-reversed", rules.LatestApplication);
        await CaptureAsync(window, "10-rule-reversed");
    }

    private static async Task AssertConfirmationEscapeAsync(MainWindow window, string confirmationId, string returnId)
    {
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        var confirm = Find<Button>(window, confirmationId);
        confirm.BringIntoView();
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        Assert.IsTrue(confirm.Focus());
        // Exercise WPF routing from a child control; this is not native keyboard acceptance.
        var escape = new KeyEventArgs(Keyboard.PrimaryDevice, PresentationSource.FromVisual(confirm), 0, Key.Escape)
        { RoutedEvent = Keyboard.PreviewKeyDownEvent };
        confirm.RaiseEvent(escape);
        Assert.IsTrue(escape.Handled, "Escape must dismiss an inline rule confirmation.");
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
        var returnTarget = Find<Button>(window, returnId);
        Assert.IsTrue(returnTarget.IsKeyboardFocused, "Cancelling must restore focus to the originating action.");
        var unrelatedEscape = new KeyEventArgs(Keyboard.PrimaryDevice, PresentationSource.FromVisual(returnTarget), 0, Key.Escape)
        { RoutedEvent = Keyboard.PreviewKeyDownEvent };
        returnTarget.RaiseEvent(unrelatedEscape);
        Assert.IsFalse(unrelatedEscape.Handled, "Rules must not consume Escape when no confirmation is open.");
    }
    private async Task ExerciseMutationAsync(MainWindow window, ShellViewModel shell, WorkerClient worker, string baselineCorpus)
    {
        // Mutation coverage owns a separate corpus; the 669-copy baseline and original sources stay intact.
        var mutationRoot = Path.Combine(_state, "mutation-corpus");
        var currentRoot = Directory.CreateDirectory(Path.Combine(mutationRoot, "Current documents")).FullName;
        var backupRoot = Directory.CreateDirectory(Path.Combine(mutationRoot, "Backup documents")).FullName;
        var source = Path.Combine(baselineCorpus, "Working library", "Project overview.md");
        var first = Path.Combine(currentRoot, "Project overview.md");
        var second = Path.Combine(backupRoot, "Archived project overview.md");
        File.Copy(source, first); File.Copy(source, second);
        File.Copy(Path.Combine(baselineCorpus, "Working library", "workspace-manifest.toml"), Path.Combine(currentRoot, "Unique manifest.toml"));
        Record("mutation-corpus", new { Source = source, Roots = new[] { currentRoot, backupRoot }, SourceSha256 = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(File.ReadAllBytes(source))) });
        await Await(shell.Sessions.NewSessionCommand.ExecuteAsync(null));
        await Until(() => !shell.Setup.IsDetectingCloudLocations, "mutation setup");
        shell.Setup.Name = "Disposable changed-file regression";
        shell.Setup.Roots[0].Path = currentRoot;
        shell.Setup.AddRootCommand.Execute(null); shell.Setup.Roots[^1].Path = backupRoot;
        await Await(shell.StartRunCommand.ExecuteAsync(null));
        await Until(() => !shell.HasActiveRun && shell.SelectedRun?.Status == "completed", "mutation baseline scan");
        var original = shell.SelectedRun!;
        Assert.AreEqual(1L, original.DuplicateFileGroups);
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        await Until(() => !shell.DuplicateFiles.IsLoading && shell.DuplicateFiles.TotalGroups == 1, "mutation file set");
        var files = shell.DuplicateFiles;
        files.SelectedGroup = files.Groups.Single();
        await Until(() => !files.IsDetailLoading && files.Members.Count == 2, "mutation copies");
        var immutableSizes = files.Members.ToDictionary(member => member.Id, member => member.Member.Size);
        var markedPath = files.Members[1].Path;
        await ExerciseRuleAsync(window, shell, worker, original, currentRoot, backupRoot);
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        await Until(() => !files.IsLoading && !files.IsDetailLoading && files.Members.Count == 2, "copies after rule reversal");
        await Await(files.KeepMemberCommand.ExecuteAsync(files.Members[0]));
        await Await(files.RemoveMemberCommand.ExecuteAsync(files.Members[1]));
        shell.SelectedDestination = WorkspaceDestination.Review;
        await Until(() => !shell.Preflight.IsLoading && shell.Preflight.HasReviewRemovals, "mutation review");
        await Await(shell.Preflight.StartCommand.ExecuteAsync(null));
        Assert.AreEqual("completed", shell.Preflight.Preflight?.Status);
        Assert.IsTrue(shell.Preflight.IsCurrent);
        var survivorPath = files.Members.Single(member => member.Path != markedPath).Path;
        // File-page validation reads metadata only. The full plan check must detect denied content reads.
        using (var lockedCopy = new FileStream(survivorPath, FileMode.Open, FileAccess.Read, FileShare.None))
        {
            await Await(shell.Preflight.StartCommand.ExecuteAsync(null));
            Assert.IsTrue(shell.Preflight.Preflight?.UnavailableCount > 0,
                "A full content check must report the locked survivor as unavailable.");
            Record("locked-copy-unavailable", survivorPath);
            await CaptureAsync(window, "13-locked-copy");
        }
        await Await(shell.Preflight.StartCommand.ExecuteAsync(null));
        Assert.AreEqual(0L, shell.Preflight.Preflight!.UnavailableCount);
        Assert.IsTrue(shell.Preflight.IsCurrent);
        Record("mutation-preflight-before", shell.Preflight.Preflight!);

        File.AppendAllText(markedPath, "\nDisposable UI journey: this copied document changed after validation.\n");
        shell.SelectedDestination = WorkspaceDestination.FileResults;
        await Until(() => !files.IsLoading && !files.IsDetailLoading && files.Members.Count == 2, "mutated copy page");
        await Await(files.ValidateVisiblePageCommand.ExecuteAsync(null));
        await Until(() => files.Members.Any(member => member.Path == markedPath && member.Member.ValidationState == "changed"), "changed-file validation");
        var changed = files.Members.Single(member => member.Path == markedPath);
        Record("mutation-changed-member", changed.Member);
        Assert.AreEqual("undecided", changed.Member.Decision, "Changed content must invalidate removal intent.");
        Assert.AreEqual(immutableSizes[changed.Id], changed.Member.Size, "Working validation must retain original scan size.");
        await CaptureAsync(window, "10-changed-copy-detected");
        shell.SelectedDestination = WorkspaceDestination.Review;
        await Until(() => !shell.Preflight.IsLoading, "stale preflight after change");
        var stale = await Await(worker.GetLatestPreflightAsync(original.Id, _deadline.Token));
        Assert.IsNotNull(stale);
        Record("mutation-preflight-after-change", stale);
        Assert.IsFalse(stale!.IsCurrent, "Changed file invalidation must make the earlier check stale.");
        Record("mutation-preflight-stale", stale);
        await CaptureAsync(window, "11-stale-review");

        shell.SelectedDestination = WorkspaceDestination.ScanSetup;
        await Until(() => shell.CanStartRun, "repeat setup");
        await Await(shell.StartRunCommand.ExecuteAsync(null));
        await Until(() => !shell.HasActiveRun && shell.SelectedRun is { Status: "completed" } run && run.Id != original.Id, "changed-content rescan");
        Assert.AreEqual(0L, shell.SelectedRun!.DuplicateFileGroups, "Changed copies must no longer form a duplicate set.");
        Assert.AreEqual(1L, (await Await(worker.GetRunAsync(original.Id, _deadline.Token))).DuplicateFileGroups, "Earlier scan summary must remain immutable.");
        Record("mutation-rescan", shell.SelectedRun);
        shell.SelectedDestination = WorkspaceDestination.History;
        await Until(() => shell.History.Runs.Count == 2, "two mutation history runs");
        shell.History.SelectedRun = shell.History.Runs.Single(item => item.Id == original.Id);
        shell.OpenScanCommand.Execute(null);
        await Until(() => !files.IsLoading && files.Run?.Id == original.Id && files.TotalGroups == 1, "immutable prior result set");
        files.SelectedGroup = files.Groups.Single();
        await Until(() => !files.IsDetailLoading && files.Members.Count == 2, "immutable original members");
        foreach (var member in files.Members) Assert.AreEqual(immutableSizes[member.Id], member.Member.Size);
        await CaptureAsync(window, "12-original-scan-retained");
        await Await(files.ValidateVisiblePageCommand.ExecuteAsync(null));
        Assert.AreEqual("present", files.Members.Single(member => member.Path == survivorPath).Member.ValidationState);

        // Move only this disposable copy outside both scan roots to exercise missing paths without deletion.
        var resolvedMarkedPath = Path.GetFullPath(markedPath).Replace(@"\\?\", string.Empty, StringComparison.Ordinal);
        Assert.IsTrue(resolvedMarkedPath.StartsWith(mutationRoot + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase));
        var quarantine = Directory.CreateDirectory(Path.Combine(mutationRoot, "Outside selected roots")).FullName;
        File.Move(markedPath, Path.Combine(quarantine, "Changed copy retained.md"));
        await Task.Delay(250, _deadline.Token); // Allow the real 100 ms filesystem hint batch to settle.
        await Await(files.ValidateVisiblePageCommand.ExecuteAsync(null));
        Assert.AreEqual("missing", files.Members.Single(member => member.Path == markedPath).Member.ValidationState);
        Record("missing-copy-detected", markedPath);
        await CaptureAsync(window, "14-missing-copy");
        var nested = Directory.CreateDirectory(Path.Combine(currentRoot, "New documents")).FullName;
        File.Copy(source, Path.Combine(nested, "Added real overview.md"));
        shell.SelectedDestination = WorkspaceDestination.ScanSetup;
        shell.Setup.AddRootCommand.Execute(null); shell.Setup.Roots[^1].Path = nested;
        await Await(shell.StartRunCommand.ExecuteAsync(null));
        await Until(() => !shell.HasActiveRun && shell.SelectedRun is { Status: "completed" } next && next.Id != original.Id && next.DuplicateFileGroups == 1, "add/missing/overlap rescan");
        Assert.AreEqual(2, shell.SelectedRun!.Parameters.Roots.Count, "Nested overlapping root must be combined with its parent.");
        Assert.AreEqual(3L, shell.SelectedRun.FilesDiscovered, "Missing copy is excluded and the new copy appears exactly once.");
        Record("add-missing-overlap-rescan", shell.SelectedRun);
        await CaptureAsync(window, "15-add-missing-overlap-rescan");
        Record("mutation-passed", "Separate disposable corpus: changed metadata/content detected, removal intent invalidated, prior check stale, rescan has no duplicates, original scan retains both members and sizes.");
    }
    private async Task Until(Func<bool> condition, string description)
    {
        var end = DateTime.UtcNow.AddSeconds(70);
        while (!condition())
        {
            if (DateTime.UtcNow > end) throw new TimeoutException($"Timed out waiting for {description}.");
            await Task.Delay(30, _deadline.Token);
        }
        await Dispatcher.Yield(DispatcherPriority.ApplicationIdle);
    }
    private async Task Await(Task task) => await task.WaitAsync(TimeSpan.FromSeconds(75), _deadline.Token);
    private async Task<T> Await<T>(Task<T> task) => await task.WaitAsync(TimeSpan.FromSeconds(75), _deadline.Token);
    private void Record(string step, object detail)
    {
        _steps.Add(new { Step = step, Utc = DateTime.UtcNow, Detail = detail });
        File.WriteAllText(Path.Combine(_state, "journey.json"), JsonSerializer.Serialize(_steps, new JsonSerializerOptions { WriteIndented = true }));
    }
    private async Task CaptureAsync(Window window, string name)
    {
        await window.Dispatcher.InvokeAsync(() => { }, DispatcherPriority.ApplicationIdle);
        window.UpdateLayout();
        var history = FindOrNull<DataGrid>(window, "RunHistoryGrid");
        if (history is { IsVisible: true, Items.Count: > 0 })
            Assert.IsTrue(history.Columns[0].ActualWidth >= 150, "History run rows must retain a readable column after deferred layout.");
        var image = new RenderTargetBitmap((int)window.ActualWidth, (int)window.ActualHeight, 96, 96, PixelFormats.Pbgra32);
        image.Render(window);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(image));
        using var output = File.Create(Path.Combine(_state, name + ".png")); encoder.Save(output);
        Record(name, new { Width = window.ActualWidth, Height = window.ActualHeight, Evidence = "Production WPF render; background loaded STA; not native input" });
    }
    private static T Find<T>(DependencyObject root, string id) where T : DependencyObject
    {
        if (root is T match && AutomationProperties.GetAutomationId(root) == id) return match;
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(root); index++)
        {
            var result = FindOrNull<T>(VisualTreeHelper.GetChild(root, index), id);
            if (result is not null) return result;
        }
        throw new InvalidOperationException($"Loaded production control {id} was not found.");
    }
    private static T? FindOrNull<T>(DependencyObject root, string id) where T : DependencyObject
    {
        if (root is T match && AutomationProperties.GetAutomationId(root) == id) return match;
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(root); index++)
            if (FindOrNull<T>(VisualTreeHelper.GetChild(root, index), id) is { } result) return result;
        return null;
    }
    private static IEnumerable<T> Descendants<T>(DependencyObject root) where T : DependencyObject
    {
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(root); index++)
        {
            var child = VisualTreeHelper.GetChild(root, index);
            if (child is T match) yield return match;
            foreach (var nested in Descendants<T>(child)) yield return nested;
        }
    }
    private static bool HasAncestor(DependencyObject child, DependencyObject ancestor)
    {
        for (var parent = VisualTreeHelper.GetParent(child); parent is not null; parent = VisualTreeHelper.GetParent(parent))
            if (ReferenceEquals(parent, ancestor)) return true;
        return false;
    }
    // Observes the production JSON store without replacing its reads or writes.
    private sealed class JourneyPresentationStore(string state) : IPresentationPreferencesStore
    {
        private readonly JsonPresentationPreferencesStore _disk = new(state);
        public PresentationPreferences Value { get; private set; } = PresentationPreferences.Default;
        public async Task<PresentationPreferences> LoadAsync(CancellationToken cancellationToken = default)
        { Value = await _disk.LoadAsync(cancellationToken); return Value; }
        public async Task SaveAsync(PresentationPreferences preferences, CancellationToken cancellationToken = default)
        { await _disk.SaveAsync(preferences, cancellationToken); Value = preferences; }
    }
    private sealed class ReadOnlyCheckConfirmation : IUserConfirmationService
    {
        public Task<bool> ConfirmAsync(string title, string message, CancellationToken cancellationToken = default) =>
            title == "Check marked copies?" && message.Contains("No files will be deleted.", StringComparison.Ordinal)
                ? Task.FromResult(true)
                : throw new InvalidOperationException($"Unexpected confirmation in non-destructive journey: {title}");
    }
}
