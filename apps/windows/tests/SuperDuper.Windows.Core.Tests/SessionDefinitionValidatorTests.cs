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
    public void UncRoot_IsBestEffortAndDefersReachabilityToWorkerStart()
    {
        var missing = $@"\\localhost\missing-{Guid.NewGuid():N}";

        var result = SessionDefinitionValidator.Validate("Network", [missing], [], []);

        Assert.IsTrue(result.IsValid);
        Assert.IsTrue(result.HasReachableRoot);
        Assert.IsTrue(result.Warnings.Any(warning => warning.Contains("UNC", StringComparison.Ordinal)));
        Assert.IsFalse(result.Warnings.Any(warning => warning.Contains("unavailable", StringComparison.OrdinalIgnoreCase)));
    }
}
