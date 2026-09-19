using SuperDuper.Windows.Core.Validation;
using SuperDuper.Windows.Core.ViewModels;

namespace SuperDuper.Windows.Core.Tests;

[TestClass]
public sealed class DisplayPathsTests
{
    [TestMethod]
    [DataRow(@"\\?\C:\Users\x\file.bin", @"C:\Users\x\file.bin")]
    [DataRow(@"\\?\d:\Data", @"d:\Data")]
    [DataRow(@"\\?\C:\", @"C:\")]
    [DataRow(@"\\?\C:", @"C:")]
    [DataRow(@"\\?\UNC\server\share\file", @"\\server\share\file")]
    [DataRow(@"\\?\unc\server\share", @"\\server\share")]
    [DataRow(@"\\?\Unc\server\share\dir\", @"\\server\share\dir\")]
    public void Plain_RemovesVerbatimDriveAndUncPrefixes(string verbatim, string expected) =>
        Assert.AreEqual(expected, DisplayPaths.Plain(verbatim));

    [TestMethod]
    [DataRow(@"C:\Users\x\file.bin")]
    [DataRow(@"\\server\share\file")]
    [DataRow(@"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\Data")]
    [DataRow(@"\\?\GLOBALROOT\Device\HarddiskVolume3\Data")]
    [DataRow(@"\\?\C:file.bin")]
    [DataRow(@"\\?\1:\Data")]
    [DataRow(@"\\.\C:\Data")]
    [DataRow(@"relative\path")]
    [DataRow(@"\\?\")]
    public void Plain_LeavesOtherSpellingsUnchanged(string path) =>
        Assert.AreEqual(path, DisplayPaths.Plain(path));

    [TestMethod]
    public void Plain_ReturnsEmptyForNullOrEmpty()
    {
        Assert.AreEqual(string.Empty, DisplayPaths.Plain(null));
        Assert.AreEqual(string.Empty, DisplayPaths.Plain(string.Empty));
    }

    [TestMethod]
    public void ValidatorMessagesNamePlainPathsButKeepTheExactRoot()
    {
        var root = @"\\?\UNC\server\share\Archive";
        var result = SessionDefinitionValidator.Validate("Archive", [root], [], []);

        CollectionAssert.AreEqual(new[] { root }, result.Roots.ToArray());
        Assert.IsTrue(result.Warnings.Any(message => message.Contains(@"\\server\share\Archive", StringComparison.Ordinal)));
        Assert.IsFalse(result.Warnings.Any(message => message.Contains(@"\\?\", StringComparison.Ordinal)));
    }
}
