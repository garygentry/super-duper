using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Validation;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class SessionSetupViewModelTests
{
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
    public async Task BrowseRootCommand_ReusesExistingBlankEditor()
    {
        var selected = Path.GetTempPath();
        var viewModel = new SessionSetupViewModel(
            new TestWorkerClient(),
            new TestFolderPicker(selected),
            new TestConfirmation(),
            _ => []);
        viewModel.BeginNew();

        await viewModel.BrowseRootCommand.ExecuteAsync(null);

        Assert.AreEqual(1, viewModel.Roots.Count);
        Assert.AreEqual(selected, viewModel.Roots[0].Path);
    }

    private static SessionSetupViewModel CreateViewModel(TestWorkerClient client) =>
        new(client, new TestFolderPicker(), new TestConfirmation(), _ => []);
}
