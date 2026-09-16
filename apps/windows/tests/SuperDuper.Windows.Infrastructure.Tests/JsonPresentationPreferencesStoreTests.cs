using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class JsonPresentationPreferencesStoreTests
{
    [TestMethod]
    public async Task LoadAsync_MissingFile_ReturnsCollapsedFilesDefaults()
    {
        var directory = CreateDirectory();
        try
        {
            var store = new JsonPresentationPreferencesStore(directory);

            var preferences = await store.LoadAsync();

            Assert.AreSame(PresentationPreferences.Default, preferences);
            Assert.IsFalse(preferences.IsSavedScanSelectorExpanded);
            Assert.AreEqual(ResultsDisplayMode.Files, preferences.LastResultsMode);
            Assert.IsFalse(preferences.IsSectionExpanded("setup.advanced"));
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(directory);
        }
    }

    [TestMethod]
    public async Task LoadAsync_CorruptOrUnsupportedDocument_ReturnsDefaults()
    {
        var directory = CreateDirectory();
        try
        {
            var path = Path.Combine(directory, "presentation-preferences.json");
            await File.WriteAllTextAsync(path, "{ not json");
            var store = new JsonPresentationPreferencesStore(directory);

            Assert.AreSame(PresentationPreferences.Default, await store.LoadAsync());

            await File.WriteAllTextAsync(path, """
                {"version":999,"isSavedScanSelectorExpanded":true,"lastResultsMode":"Folders","sectionExpansion":{}}
                """);
            Assert.AreSame(PresentationPreferences.Default, await store.LoadAsync());
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(directory);
        }
    }

    [TestMethod]
    public async Task SaveAsync_RoundTripsOnlyPresentationChoices()
    {
        var directory = CreateDirectory();
        try
        {
            var expected = PresentationPreferences.Default with
            {
                IsSavedScanSelectorExpanded = true,
                LastResultsMode = ResultsDisplayMode.Folders,
            };
            expected = expected.WithSectionExpanded("setup.advanced", true);
            expected = expected.WithSectionExpanded("results.filters", false);
            var store = new JsonPresentationPreferencesStore(directory);

            await store.SaveAsync(expected);
            var actual = await store.LoadAsync();
            var document = await File.ReadAllTextAsync(Path.Combine(directory, "presentation-preferences.json"));

            Assert.IsTrue(actual.IsSavedScanSelectorExpanded);
            Assert.AreEqual(ResultsDisplayMode.Folders, actual.LastResultsMode);
            Assert.IsTrue(actual.IsSectionExpanded("setup.advanced"));
            Assert.IsFalse(actual.IsSectionExpanded("results.filters"));
            Assert.IsFalse(document.Contains("path", StringComparison.OrdinalIgnoreCase));
            Assert.IsFalse(document.Contains("decision", StringComparison.OrdinalIgnoreCase));
            Assert.IsFalse(document.Contains("error", StringComparison.OrdinalIgnoreCase));
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(directory);
        }
    }

    [TestMethod]
    public async Task StoresWithDifferentStateDirectories_AreIsolated()
    {
        var firstDirectory = CreateDirectory();
        var secondDirectory = CreateDirectory();
        try
        {
            var first = new JsonPresentationPreferencesStore(firstDirectory);
            var second = new JsonPresentationPreferencesStore(secondDirectory);
            await first.SaveAsync(PresentationPreferences.Default with
            {
                IsSavedScanSelectorExpanded = true,
                LastResultsMode = ResultsDisplayMode.Folders,
            });

            var isolated = await second.LoadAsync();

            Assert.IsFalse(isolated.IsSavedScanSelectorExpanded);
            Assert.AreEqual(ResultsDisplayMode.Files, isolated.LastResultsMode);
        }
        finally
        {
            await TestDirectoryCleanup.DeleteAsync(firstDirectory);
            await TestDirectoryCleanup.DeleteAsync(secondDirectory);
        }
    }

    private static string CreateDirectory()
    {
        var directory = Path.Combine(Path.GetTempPath(), $"super-duper-presentation-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        return directory;
    }
}
