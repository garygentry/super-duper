using SuperDuper.Windows.Core.Validation;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class SessionDefinitionValidatorTests
{
    [TestMethod]
    public void Validate_RejectsCaseInsensitiveDuplicateName()
    {
        var result = SessionDefinitionValidator.Validate(
            " photos ",
            [Path.GetTempPath()],
            [],
            ["Photos"]);

        Assert.IsFalse(result.IsValid);
        StringAssert.Contains(string.Join(" ", result.Errors), "already uses");
    }

    [TestMethod]
    public void Validate_CollapsesDuplicateAndNestedRoots()
    {
        var parent = Directory.CreateTempSubdirectory("super-duper-parent-");
        try
        {
            var child = parent.CreateSubdirectory("child");
            var result = SessionDefinitionValidator.Validate(
                "Archives",
                [child.FullName, parent.FullName, parent.FullName.ToUpperInvariant()],
                [],
                []);

            Assert.IsTrue(result.IsValid);
            Assert.AreEqual(1, result.Roots.Count);
            Assert.AreEqual(parent.FullName, result.Roots[0], true);
            Assert.IsTrue(result.Warnings.Any(warning => warning.Contains("nested root", StringComparison.OrdinalIgnoreCase)));
        }
        finally
        {
            parent.Delete(recursive: true);
        }
    }

    [TestMethod]
    public void SafeDefaults_IncludeProtectedWindowsLocations()
    {
        CollectionAssert.Contains(SessionDefinitionValidator.SafeWindowsIgnorePatterns.ToArray(), "*/$RECYCLE.BIN");
        CollectionAssert.Contains(SessionDefinitionValidator.SafeWindowsIgnorePatterns.ToArray(), "*/System Volume Information");
    }

    [TestMethod]
    public void LocationWarnings_ExplainRemovableMappedAndUncBestEffortBehavior()
    {
        var removable = SessionDefinitionValidator.LocationWarning(
            @"E:\Archive",
            ScanRootKind.Removable,
            reachable: true);
        var mapped = SessionDefinitionValidator.LocationWarning(
            @"Z:\Team",
            ScanRootKind.MappedNetwork,
            reachable: true);
        var unc = SessionDefinitionValidator.LocationWarning(
            @"\\server\share",
            ScanRootKind.UncNetwork,
            reachable: false);

        StringAssert.Contains(removable, "disconnects");
        StringAssert.Contains(mapped, "worker process account");
        StringAssert.Contains(unc, "credentials");
        Assert.IsNull(SessionDefinitionValidator.LocationWarning(
            @"C:\Data",
            ScanRootKind.Fixed,
            reachable: true));
    }

    [TestMethod]
    public void ExtendedLocalRoot_PreservesDriveClassificationAndStoredPath()
    {
        var path = Path.GetTempPath().TrimEnd(Path.DirectorySeparatorChar);
        var extendedPath = @"\\?\" + path;
        var ordinary = SessionDefinitionValidator.Validate("Local", [path], [], []);
        var extended = SessionDefinitionValidator.Validate("Local", [extendedPath], [], []);

        Assert.AreNotEqual(ScanRootKind.Other, SessionDefinitionValidator.ClassifyRoot(path));
        Assert.AreEqual(SessionDefinitionValidator.ClassifyRoot(path),
            SessionDefinitionValidator.ClassifyRoot(extendedPath));
        Assert.IsTrue(extended.IsValid);
        Assert.IsTrue(extended.HasReachableRoot);
        Assert.AreEqual(extendedPath, extended.Roots.Single());
        Assert.AreEqual(ordinary.Warnings.Count, extended.Warnings.Count);
        Assert.IsFalse(extended.Warnings.Any(warning => warning.Contains("not been classified", StringComparison.Ordinal)));
    }

    [TestMethod]
    [DataRow(@"\\localhost\")]
    [DataRow(@"\\?\UNC\localhost\")]
    [DataRow(@"\\?\unc\localhost\")]
    public void UncRoot_IsBestEffortAndDefersReachabilityToWorkerStart(string prefix)
    {
        var missing = $"{prefix}missing-{Guid.NewGuid():N}";

        var result = SessionDefinitionValidator.Validate("Network", [missing], [], []);

        Assert.AreEqual(ScanRootKind.UncNetwork, SessionDefinitionValidator.ClassifyRoot(missing));
        Assert.IsTrue(result.IsValid);
        Assert.IsTrue(result.HasReachableRoot);
        Assert.IsTrue(result.Warnings.Any(warning => warning.Contains("UNC", StringComparison.Ordinal)));
        Assert.IsFalse(result.Warnings.Any(warning => warning.Contains("unavailable", StringComparison.OrdinalIgnoreCase)));
    }

    [TestMethod]
    public void ValidateSyntax_NeverTouchesTheFileSystem()
    {
        // A syntactically valid but nonexistent root would need a filesystem or drive call to
        // classify or check reachability; ValidateSyntax must not make that call (issue #44).
        var missing = Path.Combine(Path.GetTempPath(), Guid.NewGuid().ToString("N"));

        var result = SessionDefinitionValidator.ValidateSyntax("Setup", [missing], [], []);

        Assert.IsTrue(result.IsValid);
        Assert.IsTrue(result.HasReachableRoot, "syntax validation is optimistic until a probe narrows it");
        Assert.IsFalse(result.Warnings.Any(warning => warning.Contains("unavailable", StringComparison.OrdinalIgnoreCase)));
    }

    [TestMethod]
    public async Task ProbeRootAvailabilityAsync_DropsAProbeThatDoesNotFinishByTheDeadline()
    {
        using var release = new ManualResetEventSlim(false);
        RootAvailability BlockOnSlowRoot(string root)
        {
            if (root == "slow")
            {
                release.Wait(TimeSpan.FromSeconds(10));
            }
            return new RootAvailability(ScanRootKind.Fixed, true, null);
        }

        try
        {
            var results = await SessionDefinitionValidator.ProbeRootAvailabilityAsync(
                ["slow", "fast"],
                TimeSpan.FromMilliseconds(200),
                BlockOnSlowRoot);

            Assert.IsFalse(results.ContainsKey("slow"), "a probe past the deadline is left out, not waited on");
            Assert.IsTrue(results.ContainsKey("fast"));
            Assert.IsTrue(results["fast"].Reachable);
        }
        finally
        {
            release.Set();
        }
    }

    [TestMethod]
    public async Task ProbeRootAvailabilityAsync_ReturnsEveryRootThatFinishesInTime()
    {
        var results = await SessionDefinitionValidator.ProbeRootAvailabilityAsync(
            [Path.GetTempPath()],
            TimeSpan.FromSeconds(3));

        Assert.AreEqual(1, results.Count);
        Assert.IsTrue(results[Path.GetTempPath()].Reachable);
    }
}
