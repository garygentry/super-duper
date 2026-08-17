namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WindowsCloudLocationServiceTests
{
    [TestMethod]
    [TestCategory("RealCloudProvider")]
    public async Task DetectAsync_FindsOperatorExpectedWindowsRegistrationWithoutContentAccess()
    {
        var expectedRoot = Environment.GetEnvironmentVariable("SUPER_DUPER_EXPECTED_CLOUD_ROOT");
        if (string.IsNullOrWhiteSpace(expectedRoot))
        {
            Assert.Inconclusive(
                "Set SUPER_DUPER_EXPECTED_CLOUD_ROOT and run through Invoke-WindowsCloudPolicyAcceptance.ps1.");
        }

        var result = await new WindowsCloudLocationService().DetectAsync();

        Assert.AreEqual("complete", result.Status, result.ErrorMessage);
        Assert.IsTrue(result.Locations.Any(location => string.Equals(
            Path.TrimEndingDirectorySeparator(location.Path),
            Path.TrimEndingDirectorySeparator(expectedRoot),
            StringComparison.OrdinalIgnoreCase)));
    }

    [TestMethod]
    public async Task DetectAsync_ReturnsUnsupportedWithoutEnumeratingRegistrations()
    {
        var source = new TestSyncRootSource(isSupported: false);
        var service = new WindowsCloudLocationService(source);

        var result = await service.DetectAsync();

        Assert.AreEqual("unsupported", result.Status);
        Assert.AreEqual(0, result.Locations.Count);
        Assert.IsFalse(source.Enumerated);
    }

    [TestMethod]
    public async Task DetectAsync_ReturnsUnavailableWhenRegistrationEnumerationFails()
    {
        var service = new WindowsCloudLocationService(new TestSyncRootSource(
            error: new InvalidOperationException("provider unavailable")));

        var result = await service.DetectAsync();

        Assert.AreEqual("unavailable", result.Status);
        Assert.AreEqual(0, result.Locations.Count);
        StringAssert.Contains(result.ErrorMessage, "provider unavailable");
    }

    [TestMethod]
    public async Task DetectAsync_SkipsBlankPathsAndReturnsSortedDistinctRegistrations()
    {
        var service = new WindowsCloudLocationService(new TestSyncRootSource(
            registrations:
            [
                new StorageProviderSyncRootRegistration(@"D:\Cloud", "ProviderB!account"),
                new StorageProviderSyncRootRegistration(null, "Ignored!account"),
                new StorageProviderSyncRootRegistration(@"d:\cloud", "Duplicate!account"),
                new StorageProviderSyncRootRegistration(@"C:\Another", "ProviderA!account"),
            ]));

        var result = await service.DetectAsync();

        Assert.AreEqual("complete", result.Status);
        Assert.AreEqual(2, result.Locations.Count);
        Assert.AreEqual(@"C:\Another", result.Locations[0].Path);
        Assert.AreEqual(@"D:\Cloud", result.Locations[1].Path);
    }

    [TestMethod]
    public async Task DetectAsync_ObservesCancellationBeforeRegistrationDiscovery()
    {
        var source = new TestSyncRootSource();
        var service = new WindowsCloudLocationService(source);
        using var cancellation = new CancellationTokenSource();
        cancellation.Cancel();

        await Assert.ThrowsExceptionAsync<TaskCanceledException>(
            () => service.DetectAsync(cancellation.Token));

        Assert.IsFalse(source.Enumerated);
    }

    [TestMethod]
    public void CreateLocation_UsesRegistrationIdentityWithoutOpeningContent()
    {
        var location = WindowsCloudLocationService.CreateLocation(
            @"C:\Users\person\OneDrive",
            "OneDrive!S-1-test!Personal");

        Assert.AreEqual(@"C:\Users\person\OneDrive", location.Path);
        Assert.AreEqual("OneDrive!S-1-test!Personal", location.ProviderId);
        Assert.AreEqual("OneDrive", location.DisplayName);
    }

    [TestMethod]
    public void CreateLocation_UsesReadableFallbackForMissingProviderIdentity()
    {
        var location = WindowsCloudLocationService.CreateLocation(@"C:\Cloud", null);

        Assert.AreEqual("Cloud provider", location.DisplayName);
    }

    private sealed class TestSyncRootSource(
        bool isSupported = true,
        IReadOnlyList<StorageProviderSyncRootRegistration>? registrations = null,
        Exception? error = null) : IStorageProviderSyncRootSource
    {
        public bool Enumerated { get; private set; }

        public bool IsSupported() => isSupported;

        public IReadOnlyList<StorageProviderSyncRootRegistration> GetCurrentSyncRoots()
        {
            Enumerated = true;
            if (error is not null)
            {
                throw error;
            }
            return registrations ?? [];
        }
    }
}
