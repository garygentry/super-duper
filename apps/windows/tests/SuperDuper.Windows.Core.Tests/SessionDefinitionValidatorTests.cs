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
}
