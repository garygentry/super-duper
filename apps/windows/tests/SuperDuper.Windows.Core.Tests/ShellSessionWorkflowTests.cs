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

    private static ShellViewModel CreateShell(TestWorkerClient client) =>
        new(client, new TestFolderPicker(), new TestConfirmation(), new ImmediateDispatcher());
}
