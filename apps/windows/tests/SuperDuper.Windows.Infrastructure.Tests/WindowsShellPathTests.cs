namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WindowsShellPathTests
{
    [TestMethod]
    public void ToParsingPath_PreservesOrdinaryDosPath()
    {
        const string path = @"C:\Archive\photo.jpg";

        Assert.AreEqual(path, WindowsShellPath.ToParsingPath(path));
    }

    [TestMethod]
    public void ToParsingPath_ConvertsExtendedDosPath()
    {
        Assert.AreEqual(
            @"C:\Archive\photo.jpg",
            WindowsShellPath.ToParsingPath(@"\\?\C:\Archive\photo.jpg"));
    }

    [TestMethod]
    public void ToParsingPath_ConvertsExtendedUncPath()
    {
        Assert.AreEqual(
            @"\\server\share\Archive\photo.jpg",
            WindowsShellPath.ToParsingPath(@"\\?\UNC\server\share\Archive\photo.jpg"));
    }

    [TestMethod]
    public void ToParsingPath_PreservesLongDosPathAfterRemovingExtendedPrefix()
    {
        var local = @"C:\" + string.Join("\\", Enumerable.Repeat("long-segment", 30)) + @"\photo.jpg";

        var actual = WindowsShellPath.ToParsingPath(@"\\?\" + local);

        Assert.AreEqual(local, actual);
        Assert.IsTrue(actual.Length > 260);
    }
}
