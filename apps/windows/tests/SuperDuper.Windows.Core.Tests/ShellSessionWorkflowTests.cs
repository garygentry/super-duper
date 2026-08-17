using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class ShellSessionWorkflowTests
{
    [TestMethod]
    public async Task InitializeAsync_RestoresSessionAndInterruptedRun()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Backups", Path.GetTempPath());
        client.AddRun(session.Id, "interrupted");
        using var shell = CreateShell(client);

        await shell.InitializeAsync();

        Assert.IsTrue(shell.IsWorkspaceVisible);
        Assert.AreEqual("Backups", shell.DisplaySessionName);
        Assert.AreEqual(1, shell.History.Runs.Count);
        Assert.AreEqual("Interrupted", shell.Progress.Status);
    }

    [TestMethod]
    public async Task StartRunCommand_CreatesSessionAndShowsProgress()
    {
        var client = new TestWorkerClient();
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        var root = Directory.CreateTempSubdirectory("super-duper-shell-");
        try
        {
            await shell.Sessions.NewSessionCommand.ExecuteAsync(null);
            shell.Setup.Name = "Photos";
            shell.Setup.Roots.Clear();
            shell.Setup.Roots.Add(new SessionRootViewModel(root.FullName));

            await shell.StartRunCommand.ExecuteAsync(null);

            Assert.AreEqual(1, client.Sessions.Count);
            Assert.AreEqual(1, client.Runs.Count);
            Assert.IsTrue(shell.HasActiveRun);
            Assert.AreEqual(1, shell.SelectedTabIndex);
            Assert.AreEqual("Scanning", shell.Progress.Status);
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }

    [DataTestMethod]
    [DataRow("completed")]
    [DataRow("cancelled")]
    [DataRow("failed")]
    [DataRow("interrupted")]
    public async Task TerminalLifecycle_ImmediatelyReenablesRerunWithoutSetupEdit(string terminalStatus)
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Rerun", Path.GetTempPath());
        using var shell = CreateShell(client);
        await shell.InitializeAsync();

        await shell.StartRunCommand.ExecuteAsync(null);
        var active = client.Runs.Single();
        var terminal = active with
        {
            Status = terminalStatus,
            CompletedAt = DateTimeOffset.UtcNow,
            ErrorMessage = terminalStatus is "failed" or "interrupted" ? "Run stopped." : null,
        };
        client.RaiseLifecycle($"run.{terminalStatus}", terminal);

        Assert.IsFalse(shell.HasActiveRun);
        Assert.IsTrue(shell.Setup.CanStart);
        Assert.IsTrue(shell.StartRunCommand.CanExecute(null));
        Assert.AreEqual(active.Id, shell.History.SelectedRun?.Id);
    }

    [TestMethod]
    public async Task UnexpectedExit_OffersRestartReconcilesRunAndPreservesCompletedHistory()
    {
        var client = new TestWorkerClient();
        var session = client.AddSession("Recovery", Path.GetTempPath());
        var completed = client.AddRun(session.Id, "completed");
        using var shell = CreateShell(client);
        await shell.InitializeAsync();
        await shell.StartRunCommand.ExecuteAsync(null);
        var abandoned = client.Runs.Single(run => run.Id != completed.Id);

        client.RaiseUnexpectedExit(23);

        Assert.IsTrue(shell.IsRecoveryRequired);
        Assert.IsFalse(shell.HasActiveRun);
        Assert.AreEqual("Interrupted", shell.Progress.Status);
        Assert.IsTrue(shell.RestartWorkerCommand.CanExecute(null));

        await shell.RestartWorkerCommand.ExecuteAsync(null);

        Assert.AreEqual(1, client.RestartCount);
        Assert.IsTrue(shell.IsConnected);
        Assert.AreEqual("interrupted", client.Runs.Single(run => run.Id == abandoned.Id).Status);
        Assert.AreEqual("completed", client.Runs.Single(run => run.Id == completed.Id).Status);
        Assert.IsTrue(shell.StartRunCommand.CanExecute(null));
    }

    private static ShellViewModel CreateShell(TestWorkerClient client) =>
        new(
            client,
            new TestFolderPicker(),
            new TestConfirmation(),
            new ImmediateDispatcher(),
            new TestClipboard(),
            new TestExplorer(),
            new TestCloudLocationService());
}
