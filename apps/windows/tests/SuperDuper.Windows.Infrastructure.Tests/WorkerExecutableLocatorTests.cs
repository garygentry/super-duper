namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerExecutableLocatorTests
{
    [TestMethod]
    public void DevelopmentOverrides_AreEnabledOnlyInDebugBuilds()
    {
#if DEBUG
        Assert.IsTrue(WorkerExecutableLocator.DevelopmentOverridesEnabled);
#else
        Assert.IsFalse(WorkerExecutableLocator.DevelopmentOverridesEnabled);
#endif
    }

    [TestMethod]
    public void ReleasePolicy_IgnoresConfiguredPathEvenWhenItExists()
    {
        using var layout = new LocatorLayout();
        var install = layout.CreateDirectory("install");
        var deployed = layout.CreateWorker("install");
        var configured = layout.CreateWorker("elsewhere");

        var resolved = WorkerExecutableLocator.Resolve(install, install, configured, allowDevelopmentOverrides: false);

        Assert.AreEqual(deployed, resolved);
    }

    [TestMethod]
    public void ReleasePolicy_IgnoresRepositoryDebugWorkerWhenDeployedWorkerIsMissing()
    {
        using var layout = new LocatorLayout();
        layout.CreateRepositoryMarkers("repo");
        layout.CreateWorker(Path.Combine("repo", "target", "debug"));
        var install = layout.CreateDirectory(Path.Combine("repo", "apps", "bin"));

        var fromBase = WorkerExecutableLocator.Resolve(install, install, null, allowDevelopmentOverrides: false);
        var fromCurrent = WorkerExecutableLocator.Resolve(
            layout.CreateDirectory("install"), layout.PathOf("repo"), null, allowDevelopmentOverrides: false);

        Assert.AreEqual(Path.Combine(install, WorkerExecutableLocator.ExecutableName), fromBase);
        Assert.IsFalse(File.Exists(fromBase));
        Assert.AreEqual(
            Path.Combine(layout.PathOf("install"), WorkerExecutableLocator.ExecutableName),
            fromCurrent);
    }

    [TestMethod]
    public void DebugPolicy_HonorsConfiguredPathAsFullPath()
    {
        using var layout = new LocatorLayout();
        var install = layout.CreateDirectory("install");
        layout.CreateWorker("install");
        var configured = Path.Combine(layout.Root, "tools", "..", "custom", WorkerExecutableLocator.ExecutableName);

        var resolved = WorkerExecutableLocator.Resolve(install, install, configured, allowDevelopmentOverrides: true);

        Assert.AreEqual(Path.Combine(layout.Root, "custom", WorkerExecutableLocator.ExecutableName), resolved);
    }

    [TestMethod]
    public void DebugPolicy_PrefersDeployedWorkerOverRepositoryWorker()
    {
        using var layout = new LocatorLayout();
        layout.CreateRepositoryMarkers("repo");
        layout.CreateWorker(Path.Combine("repo", "target", "debug"));
        var install = layout.CreateDirectory(Path.Combine("repo", "apps", "bin"));
        var deployed = layout.CreateWorker(Path.Combine("repo", "apps", "bin"));

        var resolved = WorkerExecutableLocator.Resolve(install, install, "  ", allowDevelopmentOverrides: true);

        Assert.AreEqual(deployed, resolved);
    }

    [TestMethod]
    public void DebugPolicy_FallsBackToRepositoryDebugWorker()
    {
        using var layout = new LocatorLayout();
        layout.CreateRepositoryMarkers("repo");
        var development = layout.CreateWorker(Path.Combine("repo", "target", "debug"));
        var install = layout.CreateDirectory(Path.Combine("repo", "apps", "bin"));

        var fromBase = WorkerExecutableLocator.Resolve(install, install, null, allowDevelopmentOverrides: true);
        var fromCurrent = WorkerExecutableLocator.Resolve(
            layout.CreateDirectory("install"), layout.PathOf("repo"), null, allowDevelopmentOverrides: true);

        Assert.AreEqual(development, fromBase);
        Assert.AreEqual(development, fromCurrent);
    }

    private sealed class LocatorLayout : IDisposable
    {
        public LocatorLayout()
        {
            Root = Path.Combine(Path.GetTempPath(), $"super-duper-locator-{Guid.NewGuid():N}");
            Directory.CreateDirectory(Root);
        }

        public string Root { get; }

        public string PathOf(string relative) => Path.Combine(Root, relative);

        public string CreateDirectory(string relative) => Directory.CreateDirectory(PathOf(relative)).FullName;

        public string CreateWorker(string relativeDirectory)
        {
            var path = Path.Combine(CreateDirectory(relativeDirectory), WorkerExecutableLocator.ExecutableName);
            File.WriteAllBytes(path, []);
            return path;
        }

        public void CreateRepositoryMarkers(string relative)
        {
            var repository = CreateDirectory(relative);
            File.WriteAllText(Path.Combine(repository, "Cargo.toml"), string.Empty);
            Directory.CreateDirectory(Path.Combine(repository, "crates", "super-duper-worker"));
        }

        public void Dispose()
        {
            if (Directory.Exists(Root))
            {
                Directory.Delete(Root, recursive: true);
            }
        }
    }
}
