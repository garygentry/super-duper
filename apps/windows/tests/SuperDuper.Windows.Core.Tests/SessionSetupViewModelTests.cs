using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Validation;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class SessionSetupViewModelTests
{
    [TestMethod]
    public void RepeatCachePolicy_DefaultsToMeasuredReuseAndRejectsOpenValues()
    {
        var viewModel = CreateViewModel(new TestWorkerClient());

        viewModel.BeginNew();

        Assert.AreEqual(RepeatCachePolicyNames.ReuseVerified, viewModel.RepeatCachePolicy);
        Assert.AreEqual(2, viewModel.RepeatCachePolicies.Count);
        StringAssert.Contains(viewModel.RepeatCachePolicyDescription, "file identity and change metadata qualify");
        Assert.ThrowsExactly<ArgumentOutOfRangeException>(
            () => viewModel.RepeatCachePolicy = "trust_path");
    }

    [TestMethod]
    public void RepeatCachePolicy_AlternateDisclosureExplainsContentReads()
    {
        var viewModel = CreateViewModel(new TestWorkerClient());

        viewModel.RepeatCachePolicy = RepeatCachePolicyNames.RevalidateContent;

        StringAssert.Contains(viewModel.RepeatCachePolicyDescription, "normal candidate filtering");
        StringAssert.Contains(viewModel.RepeatCachePolicyDescription, "does not force a full hash");
    }

    [TestMethod]
    public void BeginNew_SeedsSafeWindowsIgnorePatterns()
    {
        var viewModel = CreateViewModel(new TestWorkerClient());

        viewModel.BeginNew();

        foreach (var pattern in SessionDefinitionValidator.SafeWindowsIgnorePatterns)
        {
            StringAssert.Contains(viewModel.IgnorePatternsText, pattern);
        }
    }

    [TestMethod]
    public async Task EnsureSavedAsync_NormalizesNestedRootsBeforeCreate()
    {
        var client = new TestWorkerClient();
        var viewModel = CreateViewModel(client);
        var parent = Directory.CreateTempSubdirectory("super-duper-session-");
        try
        {
            var child = parent.CreateSubdirectory("child");
            viewModel.BeginNew();
            viewModel.Name = "Archives";
            viewModel.Roots.Clear();
            viewModel.Roots.Add(new SessionRootViewModel(child.FullName));
            viewModel.Roots.Add(new SessionRootViewModel(parent.FullName));

            var saved = await viewModel.EnsureSavedAsync(requireReachableRoot: true);

            Assert.IsNotNull(saved);
            Assert.AreEqual(1, saved.Roots.Count);
            Assert.AreEqual(parent.FullName, saved.Roots[0], true);
        }
        finally
        {
            parent.Delete(recursive: true);
        }
    }

    [TestMethod]
    public void Validation_DisablesStartForUnavailableRoots()
    {
        var viewModel = CreateViewModel(new TestWorkerClient());
        viewModel.BeginNew();
        viewModel.Roots.Clear();
        viewModel.Roots.Add(new SessionRootViewModel(Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"))));

        Assert.IsFalse(viewModel.CanStart);
        Assert.IsTrue(viewModel.HasWarnings);
    }

    [TestMethod]
    public async Task EnsureSavedAsync_RenamesAndUpdatesExistingSession()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Old name", Path.GetTempPath());
        var viewModel = CreateViewModel(client);
        viewModel.Load(session);

        viewModel.Name = "Renamed";
        var saved = await viewModel.EnsureSavedAsync(requireReachableRoot: false);

        Assert.IsNotNull(saved);
        Assert.AreEqual("Renamed", client.Sessions.Single().Name);
    }

    [TestMethod]
    public async Task EnsureSavedAsync_PreservesTheSelectedRepeatCachePolicyAcrossSaveThenStart()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Old name", Path.GetTempPath());
        var viewModel = CreateViewModel(client);
        viewModel.Load(session);

        viewModel.RepeatCachePolicy = RepeatCachePolicyNames.RevalidateContent;
        viewModel.Name = "Renamed";
        var saved = await viewModel.EnsureSavedAsync(requireReachableRoot: false);

        Assert.IsNotNull(saved);
        Assert.AreEqual(RepeatCachePolicyNames.RevalidateContent, viewModel.RepeatCachePolicy);
    }

    [TestMethod]
    public void BeginNew_DefaultsToReuseVerifiedEvenAfterSelectingRevalidateContent()
    {
        var viewModel = CreateViewModel(new TestWorkerClient());
        viewModel.BeginNew();
        viewModel.RepeatCachePolicy = RepeatCachePolicyNames.RevalidateContent;

        viewModel.BeginNew();

        Assert.AreEqual(RepeatCachePolicyNames.ReuseVerified, viewModel.RepeatCachePolicy);
    }

    [TestMethod]
    public async Task DeleteCommand_RemovesPersistedSessionAfterConfirmation()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Disposable", Path.GetTempPath());
        var viewModel = CreateViewModel(client);
        viewModel.Load(session);
        long? deletedId = null;
        viewModel.SessionDeleted += (_, id) => deletedId = id;

        await viewModel.DeleteCommand.ExecuteAsync(null);

        Assert.AreEqual(session.Id, deletedId);
        Assert.AreEqual(0, client.Sessions.Count);
    }

    [TestMethod]
    public void AddRootCommand_KeepsAtMostOneBlankEditor()
    {
        var viewModel = CreateViewModel(new TestWorkerClient());
        viewModel.BeginNew();

        viewModel.AddRootCommand.Execute(null);
        viewModel.AddRootCommand.Execute(null);

        Assert.AreEqual(1, viewModel.Roots.Count(root => string.IsNullOrWhiteSpace(root.Path)));
    }

    [TestMethod]
    public void VerbatimRootStatusNamesThePlainPathAndKeepsTheStoredRoot()
    {
        const string root = @"\\?\UNC\server\share\Archive";
        var viewModel = new SessionSetupViewModel(
            new TestWorkerClient(),
            new TestFolderPicker(),
            new TestConfirmation(),
            _ => [],
            new TestCloudLocationService());
        viewModel.BeginNew();

        viewModel.Roots[0].Path = root;

        Assert.AreEqual(root, viewModel.Roots[0].Path);
        Assert.IsTrue(viewModel.Roots[0].HasStatusNotice);
        StringAssert.Contains(viewModel.Roots[0].Status, @"UNC network root is best-effort");
        StringAssert.Contains(viewModel.Roots[0].Status, @"\\server\share\Archive");
        Assert.IsFalse(viewModel.Roots[0].Status.Contains(@"\\?\", StringComparison.Ordinal));
    }

    [TestMethod]
    public async Task BrowseRootCommand_ReusesExistingBlankEditor()
    {
        var selected = Path.GetTempPath();
        var viewModel = new SessionSetupViewModel(
            new TestWorkerClient(),
            new TestFolderPicker(selected),
            new TestConfirmation(),
            _ => [],
            new TestCloudLocationService());
        viewModel.BeginNew();

        await viewModel.BrowseRootCommand.ExecuteAsync(null);

        Assert.AreEqual(1, viewModel.Roots.Count);
        Assert.AreEqual(selected, viewModel.Roots[0].Path);
    }

    [TestMethod]
    public async Task RefreshCloudLocations_ExplainsSelectedRootInsideRegisteredLocation()
    {
        var cloudRoot = Directory.CreateTempSubdirectory("super-duper-cloud-");
        try
        {
            var selected = cloudRoot.CreateSubdirectory("selected");
            var detector = new TestCloudLocationService(
                locations:
                [
                    new WorkerRegisteredCloudLocation(
                        cloudRoot.FullName,
                        "TestProvider!account",
                        "TestProvider"),
                ]);
            var viewModel = new SessionSetupViewModel(
                new TestWorkerClient(),
                new TestFolderPicker(),
                new TestConfirmation(),
                _ => [],
                detector);
            viewModel.BeginNew();
            viewModel.Roots[0].Path = selected.FullName;

            await viewModel.RefreshCloudLocationsCommand.ExecuteAsync(null);

            Assert.IsTrue(viewModel.IsCloudDetectionReady);
            Assert.AreEqual(1, viewModel.DetectedCloudLocations.Count);
            StringAssert.Contains(viewModel.DetectedCloudLocations[0].Behavior, "fully excluded");
        }
        finally
        {
            cloudRoot.Delete(recursive: true);
        }
    }

    [TestMethod]
    public async Task UnavailableCloudDetection_FailsClosedBeforeStartSave()
    {
        var root = Directory.CreateTempSubdirectory("super-duper-local-");
        try
        {
            var viewModel = new SessionSetupViewModel(
                new TestWorkerClient(),
                new TestFolderPicker(),
                new TestConfirmation(),
                _ => [],
                new TestCloudLocationService(
                    CloudDetectionStatusNames.Unavailable,
                    errorMessage: "Provider enumeration failed."));
            viewModel.BeginNew();
            viewModel.Name = "Cloud-safe";
            viewModel.Roots[0].Path = root.FullName;

            var saved = await viewModel.EnsureSavedAsync(requireReachableRoot: true);

            Assert.IsNull(saved);
            Assert.IsTrue(viewModel.HasOperationError);
            Assert.IsFalse(viewModel.CanStart);
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    [TestMethod]
    public async Task RefreshCloudLocations_NotifiesCanStartWhenDetectionBecomesReady()
    {
        var root = Directory.CreateTempSubdirectory("super-duper-local-");
        try
        {
            var detector = new TestCloudLocationService(
                CloudDetectionStatusNames.Unavailable,
                errorMessage: "Provider enumeration failed.");
            var viewModel = new SessionSetupViewModel(
                new TestWorkerClient(),
                new TestFolderPicker(),
                new TestConfirmation(),
                _ => [],
                detector);
            viewModel.BeginNew();
            viewModel.Name = "Cloud-safe";
            viewModel.Roots[0].Path = root.FullName;
            var canStartNotifications = 0;
            viewModel.PropertyChanged += (_, args) =>
            {
                if (args.PropertyName == nameof(SessionSetupViewModel.CanStart))
                {
                    canStartNotifications++;
                }
            };

            detector.Result = new CloudLocationDetectionResult(
                CloudDetectionStatusNames.Complete,
                []);
            await viewModel.RefreshCloudLocationsCommand.ExecuteAsync(null);

            Assert.IsTrue(viewModel.CanStart);
            Assert.IsTrue(canStartNotifications > 0);
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    [TestMethod]
    public async Task CloudDetectionChange_IsNotAnEditButIsSavedWhenScanStarts()
    {
        var root = Directory.CreateTempSubdirectory("super-duper-cloud-change-");
        try
        {
            var client = new TestWorkerClient();
            var saved = await client.CreateSessionAsync(
                "Saved", [root.FullName], [], CloudPolicyNames.ExcludeRegisteredRoots, [], [],
                CloudDetectionStatusNames.Complete);
            var cloud = new WorkerRegisteredCloudLocation(
                Path.Combine(root.FullName, "OneDrive"), "OneDrive!account", "OneDrive");
            var detector = new TestCloudLocationService(CloudDetectionStatusNames.Complete, [cloud]);
            var viewModel = new SessionSetupViewModel(
                client, new TestFolderPicker(), new TestConfirmation(), _ => [], detector);
            viewModel.Load(saved);

            await viewModel.RefreshCloudLocationsCommand.ExecuteAsync(null);

            Assert.IsFalse(viewModel.IsDirty, "Detection alone must not prompt to save on leave.");
            Assert.IsTrue(viewModel.HasUnsavedCloudDetection);
            Assert.IsTrue(viewModel.CanSave);
            StringAssert.Contains(viewModel.CloudDetectionSummary, "changed since this scan was saved");

            var started = await viewModel.EnsureSavedAsync(requireReachableRoot: true);

            Assert.IsNotNull(started);
            CollectionAssert.AreEqual(new[] { cloud }, client.Sessions.Single().RegisteredCloudLocations.ToArray());
            Assert.IsFalse(viewModel.HasUnsavedCloudDetection);
            Assert.IsFalse(viewModel.IsDirty);
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    [TestMethod]
    public async Task UnchangedCloudDetection_LeavesSetupClean()
    {
        var root = Directory.CreateTempSubdirectory("super-duper-cloud-same-");
        try
        {
            var client = new TestWorkerClient();
            var saved = await client.CreateSessionAsync(
                "Saved", [root.FullName], [], CloudPolicyNames.ExcludeRegisteredRoots, [], [],
                CloudDetectionStatusNames.Complete);
            var viewModel = new SessionSetupViewModel(
                client, new TestFolderPicker(), new TestConfirmation(), _ => [], new TestCloudLocationService());
            viewModel.Load(saved);

            await viewModel.RefreshCloudLocationsCommand.ExecuteAsync(null);

            Assert.IsFalse(viewModel.IsDirty);
            Assert.IsFalse(viewModel.HasUnsavedCloudDetection);
            Assert.IsFalse(viewModel.CanSave);
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    private static SessionSetupViewModel CreateViewModel(TestWorkerClient client) =>
        new(client, new TestFolderPicker(), new TestConfirmation(), _ => [], new TestCloudLocationService());
}
