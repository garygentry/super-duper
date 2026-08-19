using System.Diagnostics;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerClientLifecycleTests
{
    [TestMethod]
    public async Task DisposeAsync_WithConcurrentRequestsStopsOwnedWorker()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-concurrent-shutdown-{Guid.NewGuid():N}");
        Directory.CreateDirectory(temp);
        var client = new WorkerClient(
            worker,
            TimeSpan.FromSeconds(10),
            Path.Combine(temp, "worker.db"),
            Path.Combine(temp, "logs", "worker.log"),
            Path.Combine(temp, "hash-cache"));

        try
        {
            _ = await client.ConnectAsync();
            _ = await client.CreateSessionAsync("Concurrent shutdown", [temp], []);
            var processId = client.OwnedProcessId;
            var requests = Enumerable.Range(0, 250)
                .Select(_ => client.ListSessionsAsync())
                .ToArray();

            await client.DisposeAsync().AsTask().WaitAsync(TimeSpan.FromSeconds(10));

            Assert.IsNotNull(processId);
            Assert.ThrowsException<ArgumentException>(() => Process.GetProcessById(processId.Value));
            await Task.WhenAll(requests.Select(async request =>
            {
                try
                {
                    await request;
                }
                catch (Exception exception) when (
                    exception is ObjectDisposedException or IOException
                    || exception is WorkerProtocolException protocolException
                        && protocolException.Message.Contains("stdin is unavailable", StringComparison.Ordinal))
                {
                }
            })).WaitAsync(TimeSpan.FromSeconds(10));
        }
        finally
        {
            await client.DisposeAsync();
            if (Directory.Exists(temp))
            {
                await TestDirectoryCleanup.DeleteAsync(temp);
            }
        }
    }

    [TestMethod]
    public async Task DisposeAsync_DuringActiveRunStopsOwnedWorkerAndPersistsCancellation()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-active-shutdown-{Guid.NewGuid():N}");
        var root = Path.Combine(temp, "root");
        var database = Path.Combine(temp, "worker.db");
        Directory.CreateDirectory(root);
        for (var index = 0; index < 1_500; index++)
        {
            await File.WriteAllBytesAsync(Path.Combine(root, $"{index:D4}.bin"), new byte[4096]);
        }

        try
        {
            var client = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                database,
                Path.Combine(temp, "logs", "worker.log"),
                Path.Combine(temp, "hash-cache"));
            _ = await client.ConnectAsync();
            var session = await client.CreateSessionAsync("Active shutdown", [root], []);
            var run = await client.StartRunAsync(session.Id);
            var processId = client.OwnedProcessId;

            await client.DisposeAsync();

            Assert.IsNotNull(processId);
            Assert.ThrowsException<ArgumentException>(() => Process.GetProcessById(processId.Value));

            await using var restarted = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                database,
                Path.Combine(temp, "logs", "restarted.log"),
                Path.Combine(temp, "hash-cache"));
            _ = await restarted.ConnectAsync();
            var durable = await restarted.GetRunAsync(run.Id);
            Assert.AreEqual("cancelled", durable.Status);
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                await TestDirectoryCleanup.DeleteAsync(temp);
            }
        }
    }

    [TestMethod]
    public async Task TypedClient_CreatesSessionRunsScanAndObservesDurableCompletion()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-worker-test-{Guid.NewGuid():N}");
        var root = Path.Combine(temp, "root");
        Directory.CreateDirectory(root);
        await File.WriteAllTextAsync(Path.Combine(root, "one.txt"), "non-empty");
        await File.WriteAllTextAsync(Path.Combine(root, "one-copy.JPG"), "non-empty");
        var folderA = Directory.CreateDirectory(Path.Combine(root, "folder-a"));
        var folderB = Directory.CreateDirectory(Path.Combine(root, "folder-b"));
        await File.WriteAllTextAsync(Path.Combine(folderA.FullName, "same.txt"), "folder content");
        await File.WriteAllTextAsync(Path.Combine(folderB.FullName, "same.txt"), "folder content");
        var diagnostics = Path.Combine(temp, "logs", "worker.log");

        try
        {
            await using var client = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                Path.Combine(temp, "worker.db"),
                diagnostics,
                Path.Combine(temp, "hash-cache"));
            var terminal = new TaskCompletionSource<string>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            client.RunLifecycleChanged += (_, eventArgs) =>
            {
                if (eventArgs.EventName is "run.completed" or "run.cancelled" or "run.failed")
                {
                    terminal.TrySetResult(eventArgs.EventName);
                }
            };

            _ = await client.ConnectAsync();
            var session = await client.CreateSessionAsync("Lifecycle", [root], []);
            var sessions = await client.ListSessionsAsync();
            var started = await client.StartRunAsync(session.Id);
            var terminalEvent = await terminal.Task.WaitAsync(TimeSpan.FromSeconds(30));
            var durable = await client.GetRunAsync(started.Id);
            var groups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(string.Empty, "0")));
            var acrossDriveGroups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(string.Empty, "0", AcrossDrives: true)));
            var threeCopyGroups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.CopyCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(string.Empty, "0", MinimumCopyCount: 3)));
            var rootFacets = await client.GetDuplicateFileSelectedRootFacetsAsync(
                new DuplicateFileSelectedRootFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileSelectedRootFacetFilter(string.Empty, "0")));
            var driveFacets = await client.GetDuplicateFileDriveFacetsAsync(
                new DuplicateFileDriveFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileDriveFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileDriveFacetFilter(string.Empty, "0")));
            var threeCopyRootFacets = await client.GetDuplicateFileSelectedRootFacetsAsync(
                new DuplicateFileSelectedRootFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileSelectedRootFacetFilter(
                        string.Empty,
                        "0",
                        MinimumCopyCount: 3)));
            var threeCopyDriveFacets = await client.GetDuplicateFileDriveFacetsAsync(
                new DuplicateFileDriveFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileDriveFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileDriveFacetFilter(
                        string.Empty,
                        "0",
                        MinimumCopyCount: 3)));
            var selectedRootGroups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(
                        string.Empty,
                        "0",
                        SelectedRoot: rootFacets.Facets.Single().Value)));
            var members = await client.GetDuplicateFileGroupMembersAsync(
                new DuplicateFileMemberQuery(
                    started.Id,
                    groups.Groups.Single(group => group.RepresentativeName.StartsWith("one", StringComparison.Ordinal)).Id,
                    200,
                    DuplicateFileMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFileMemberFilter(string.Empty)));
            var reviewBefore = await client.GetReviewPlanAsync(started.Id);
            var reviewGroups = await client.GetReviewGroupsAsync(started.Id, 200);
            var reviewedMember = members.Members.Single(member => member.FileName == "one.txt");
            var operationId = $"lifecycle-{Guid.NewGuid():N}";
            var mutation = await client.SetReviewDecisionAsync(
                operationId,
                started.Id,
                reviewedMember.GroupId,
                reviewedMember.Id,
                "remove",
                reviewBefore.Plan.Revision);
            var replay = await client.SetReviewDecisionAsync(
                operationId,
                started.Id,
                reviewedMember.GroupId,
                reviewedMember.Id,
                "remove",
                reviewBefore.Plan.Revision);
            var reviewedMembers = await client.GetDuplicateFileGroupMembersAsync(
                new DuplicateFileMemberQuery(
                    started.Id,
                    reviewedMember.GroupId,
                    200,
                    DuplicateFileMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFileMemberFilter(string.Empty)));
            var reviewAfter = await client.GetReviewPlanAsync(started.Id);
            var stale = await Assert.ThrowsExceptionAsync<WorkerProtocolException>(() =>
                client.SetReviewDecisionAsync(
                    $"stale-{Guid.NewGuid():N}",
                    started.Id,
                    reviewedMember.GroupId,
                    members.Members.Single(member => member.Id != reviewedMember.Id).Id,
                    "keep",
                    reviewBefore.Plan.Revision));
            var exactPath = members.Members
                .Single(member => member.FileName == "one.txt")
                .Path.ToUpperInvariant();
            var exactPathGroups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(
                        exactPath,
                        "0",
                        PathMatch: DuplicateFilePathMatchMode.Exact)));
            var exactPathRootFacets = await client.GetDuplicateFileSelectedRootFacetsAsync(
                new DuplicateFileSelectedRootFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileSelectedRootFacetFilter(
                        exactPath,
                        "0",
                        PathMatch: DuplicateFilePathMatchMode.Exact)));
            var exactPathDriveFacets = await client.GetDuplicateFileDriveFacetsAsync(
                new DuplicateFileDriveFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileDriveFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileDriveFacetFilter(
                        exactPath,
                        "0",
                        PathMatch: DuplicateFilePathMatchMode.Exact)));
            var extensionGroups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(
                        string.Empty,
                        "0",
                        Extension: "JPG")));
            var extensionRootFacets = await client.GetDuplicateFileSelectedRootFacetsAsync(
                new DuplicateFileSelectedRootFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileSelectedRootFacetFilter(
                        string.Empty,
                        "0",
                        Extension: "JPG")));
            var extensionDriveFacets = await client.GetDuplicateFileDriveFacetsAsync(
                new DuplicateFileDriveFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileDriveFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileDriveFacetFilter(
                        string.Empty,
                        "0",
                        Extension: "JPG")));
            var allExtensionGroups = await client.GetDuplicateFileGroupsAsync(
                new DuplicateFileGroupQuery(
                    started.Id,
                    200,
                    DuplicateFileGroupSortField.RecoverableBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFileGroupFilter(
                        string.Empty,
                        "0",
                        Extension: "JPG",
                        ExtensionMatch: DuplicateFileExtensionMatchMode.AllMembers)));
            var allExtensionRootFacets = await client.GetDuplicateFileSelectedRootFacetsAsync(
                new DuplicateFileSelectedRootFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileSelectedRootFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileSelectedRootFacetFilter(
                        string.Empty,
                        "0",
                        Extension: "JPG",
                        ExtensionMatch: DuplicateFileExtensionMatchMode.AllMembers)));
            var allExtensionDriveFacets = await client.GetDuplicateFileDriveFacetsAsync(
                new DuplicateFileDriveFacetQuery(
                    started.Id,
                    25,
                    DuplicateFileDriveFacetSortField.MatchingGroupCount,
                    WorkerSortDirection.Descending,
                    new DuplicateFileDriveFacetFilter(
                        string.Empty,
                        "0",
                        Extension: "JPG",
                        ExtensionMatch: DuplicateFileExtensionMatchMode.AllMembers)));
            var folderGroups = await client.GetDuplicateFolderGroupsAsync(
                new DuplicateFolderGroupQuery(
                    started.Id,
                    200,
                    DuplicateFolderGroupSortField.TotalBytes,
                    WorkerSortDirection.Descending,
                    new DuplicateFolderGroupFilter(string.Empty, "0")));
            var folderMembers = await client.GetDuplicateFolderGroupMembersAsync(
                new DuplicateFolderMemberQuery(
                    started.Id,
                    folderGroups.Groups.Single().Id,
                    200,
                    DuplicateFolderMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFolderMemberFilter(string.Empty)));
            var folderReviewGroups = await client.GetReviewFolderGroupsAsync(started.Id, 200);
            var keptFolder = folderMembers.Members.First();
            var folderOperationId = $"folder-lifecycle-{Guid.NewGuid():N}";
            var folderMutation = await client.SetReviewFolderDecisionAsync(
                folderOperationId,
                started.Id,
                keptFolder.GroupId,
                keptFolder.Id,
                "keep",
                reviewAfter.Plan.Revision);
            var folderReplay = await client.SetReviewFolderDecisionAsync(
                folderOperationId,
                started.Id,
                keptFolder.GroupId,
                keptFolder.Id,
                "keep",
                reviewAfter.Plan.Revision);
            var reviewedFolderMembers = await client.GetDuplicateFolderGroupMembersAsync(
                new DuplicateFolderMemberQuery(
                    started.Id,
                    keptFolder.GroupId,
                    200,
                    DuplicateFolderMemberSortField.Path,
                    WorkerSortDirection.Ascending,
                    new DuplicateFolderMemberFilter(string.Empty)));
            var combinedAfterFolder = await client.GetReviewPlanAsync(started.Id);
            var missingRoot = Path.Combine(temp, "missing-root");
            var immutableRoot = rootFacets.Facets.Single().Value;
            var savedRule = await client.SavePreferenceRuleAsync(
                $"preference-{Guid.NewGuid():N}",
                null,
                "Preferred libraries",
                [immutableRoot, missingRoot],
                0);
            var savedRules = await client.ListPreferenceRulesAsync();
            var loadedRule = await client.GetPreferenceRuleAsync(savedRule.Rule.Id);
            var preferencePreview = await client.GetPreferencePreviewAsync(
                new PreferencePreviewQuery(
                    started.Id,
                    loadedRule.Id,
                    loadedRule.Revision,
                    combinedAfterFolder.Plan.Revision,
                    1,
                    new PreferencePreviewScope(PreferencePreviewScopeKind.CompletedRun)));
            var updatedRule = await client.SavePreferenceRuleAsync(
                $"preference-update-{Guid.NewGuid():N}",
                loadedRule.Id,
                loadedRule.Name,
                [missingRoot, immutableRoot],
                loadedRule.Revision);
            var stalePreferenceRule = await Assert.ThrowsExceptionAsync<WorkerProtocolException>(() =>
                client.GetPreferencePreviewAsync(
                    new PreferencePreviewQuery(
                        started.Id,
                        updatedRule.Rule.Id,
                        loadedRule.Revision,
                        combinedAfterFolder.Plan.Revision,
                        1,
                        new PreferencePreviewScope(PreferencePreviewScopeKind.CompletedRun),
                        preferencePreview.NextCursor)));

            Assert.AreEqual(1, sessions.Total);
            Assert.AreEqual("run.completed", terminalEvent);
            Assert.AreEqual(0, acrossDriveGroups.Total);
            Assert.AreEqual(0, threeCopyGroups.Total);
            Assert.AreEqual(0, threeCopyRootFacets.Total);
            Assert.AreEqual(0, threeCopyDriveFacets.Total);
            Assert.AreEqual(1, rootFacets.Total);
            Assert.AreEqual(2, rootFacets.Facets.Single().MatchingGroupCount);
            Assert.AreEqual(1, driveFacets.Total);
            Assert.AreEqual(2, driveFacets.Facets.Single().MatchingGroupCount);
            Assert.AreEqual(2, selectedRootGroups.Total);
            Assert.AreEqual("completed", durable.Status);
            Assert.AreEqual(session.Id, durable.SessionId);
            Assert.AreEqual(2, groups.Total);
            Assert.AreEqual(1, groups.Summary.DistinctSelectedRootCount);
            Assert.AreEqual(1, groups.Summary.DistinctDriveCount);
            Assert.AreEqual(0, groups.Summary.AcrossDriveGroupCount);
            Assert.IsTrue(members.Total >= 2);
            Assert.IsNull(reviewBefore.Plan.Id);
            Assert.IsTrue(reviewBefore.Summary.UndecidedCount >= members.Total);
            Assert.AreEqual(2, reviewGroups.Total);
            Assert.AreEqual(1, mutation.AppliedRevision);
            Assert.IsFalse(mutation.Replayed);
            Assert.IsTrue(replay.Replayed);
            Assert.AreEqual("remove", reviewedMembers.Members.Single(member => member.Id == reviewedMember.Id).Decision);
            Assert.AreEqual(1, reviewedMembers.ReviewSummary.RemoveCount);
            Assert.IsTrue(reviewedMembers.ReviewSummary.RemainingPhysicalCopyCount >= 1);
            Assert.AreEqual(1, reviewAfter.Summary.RemoveCount);
            Assert.AreEqual("review_generation_conflict", stale.Code);
            Assert.AreEqual(1, exactPathGroups.Total);
            Assert.AreEqual(1, exactPathGroups.Summary.MatchingGroupCount);
            Assert.AreEqual(1, exactPathRootFacets.Total);
            Assert.AreEqual(1, exactPathRootFacets.Facets.Single().MatchingGroupCount);
            Assert.AreEqual(1, exactPathDriveFacets.Total);
            Assert.AreEqual(1, exactPathDriveFacets.Facets.Single().MatchingGroupCount);
            Assert.AreEqual(1, extensionGroups.Total);
            Assert.AreEqual(1, extensionGroups.Summary.MatchingGroupCount);
            Assert.AreEqual(1, extensionRootFacets.Total);
            Assert.AreEqual(1, extensionRootFacets.Facets.Single().MatchingGroupCount);
            Assert.AreEqual(1, extensionDriveFacets.Total);
            Assert.AreEqual(1, extensionDriveFacets.Facets.Single().MatchingGroupCount);
            Assert.AreEqual(0, allExtensionGroups.Total);
            Assert.AreEqual(0, allExtensionGroups.Summary.MatchingGroupCount);
            Assert.AreEqual(0, allExtensionRootFacets.Total);
            Assert.AreEqual(0, allExtensionDriveFacets.Total);
            CollectionAssert.IsSubsetOf(
                new[] { "one.txt", "one-copy.JPG" },
                members.Members.Select(member => member.FileName).ToArray());
            Assert.AreEqual(1, folderGroups.Total);
            Assert.AreEqual(2, folderMembers.Total);
            Assert.AreEqual(1, folderReviewGroups.Total);
            Assert.AreEqual(reviewAfter.Plan.Revision + 1, folderMutation.AppliedRevision);
            Assert.IsTrue(folderReplay.Replayed);
            Assert.AreEqual("keep", reviewedFolderMembers.Members.Single(member => member.Id == keptFolder.Id).Decision);
            Assert.AreEqual(1, reviewedFolderMembers.ReviewSummary.KeepCount);
            Assert.AreEqual(folderMutation.AppliedRevision, reviewedFolderMembers.ReviewRevision);
            Assert.AreEqual(1, combinedAfterFolder.Summary.FolderKeepCount);
            Assert.AreEqual(1, savedRules.Total);
            Assert.AreEqual(2, loadedRule.Roots.Count);
            Assert.AreEqual(immutableRoot, loadedRule.Roots[0]);
            Assert.AreEqual(2, preferencePreview.Total);
            Assert.AreEqual(2, preferencePreview.Summary.AffectedGroupCount);
            Assert.AreEqual(1, preferencePreview.Summary.MissingRuleRootCount);
            Assert.IsNotNull(preferencePreview.NextCursor);
            Assert.AreEqual("preference_rule_generation_conflict", stalePreferenceRule.Code);
            CollectionAssert.AreEquivalent(
                new[] { folderA.Name, folderB.Name },
                folderMembers.Members
                    .Select(member => Path.GetFileName(member.Path.TrimEnd(Path.DirectorySeparatorChar)))
                    .ToArray());

            var diagnosticText = await WaitForDiagnosticsAsync(diagnostics);
            foreach (var phase in new[] { "discovering", "hashing", "persisting", "analyzing_folders", "finalizing" })
            {
                StringAssert.Contains(diagnosticText, $"kind=scan_phase run_id={started.Id} phase={phase}");
            }
            foreach (var method in new[]
                     {
                         "duplicate_file_group.page",
                         "duplicate_file_selected_root_facet.page",
                         "duplicate_file_drive_facet.page",
                         "duplicate_file_group.members",
                         "review_plan.get",
                         "review_group.page",
                         "review_folder_group.page",
                         "preference_rule.preview",
                         "duplicate_folder_group.page",
                         "duplicate_folder_group.members",
                     })
            {
                StringAssert.Contains(diagnosticText, $"kind=result_query method={method}");
            }
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                await TestDirectoryCleanup.DeleteAsync(temp);
            }
        }
    }

    private static async Task<string> WaitForDiagnosticsAsync(string path)
    {
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(5);
        while (DateTime.UtcNow < deadline)
        {
            if (File.Exists(path))
            {
                await using var stream = new FileStream(
                    path,
                    FileMode.Open,
                    FileAccess.Read,
                    FileShare.ReadWrite | FileShare.Delete,
                    bufferSize: 4096,
                    useAsync: true);
                using var reader = new StreamReader(stream);
                var text = await reader.ReadToEndAsync();
                if (text.Contains("duplicate_folder_group.members", StringComparison.Ordinal))
                {
                    return text;
                }
            }
            await Task.Delay(50);
        }
        Assert.Fail($"Timed out waiting for worker diagnostics at {path}.");
        return string.Empty;
    }

    private static string FindWorker()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, "target", BuildProfile, "super-duper-worker.exe");
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        Assert.Inconclusive("Build the Rust workspace before running Windows integration tests.");
        return string.Empty;
    }

#if DEBUG
    private const string BuildProfile = "debug";
#else
    private const string BuildProfile = "release";
#endif
}
