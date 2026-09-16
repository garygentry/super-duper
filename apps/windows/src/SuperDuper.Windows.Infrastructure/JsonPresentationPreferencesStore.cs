using System.Collections.ObjectModel;
using System.Text.Json;
using System.Text.Json.Serialization;
using SuperDuper.Windows.Core.Services;

namespace SuperDuper.Windows.Infrastructure;

/// <summary>
/// A small versioned JSON file for display preferences. It is intentionally separate from worker
/// databases so presentation changes cannot affect scan, decision, or file data.
/// </summary>
public sealed class JsonPresentationPreferencesStore : IPresentationPreferencesStore
{
    private const string FileName = "presentation-preferences.json";
    private const int MaximumDocumentBytes = 16 * 1024;

    private static readonly JsonSerializerOptions SerializerOptions = new(JsonSerializerDefaults.Web)
    {
        Converters = { new JsonStringEnumConverter() },
    };

    private readonly SemaphoreSlim _writeGate = new(1, 1);
    private readonly string _path;

    /// <summary>
    /// Creates a store below <paramref name="stateDirectory"/>. Supply a private directory for
    /// development and test state; omit it for the normal per-user location.
    /// </summary>
    public JsonPresentationPreferencesStore(string? stateDirectory = null)
    {
        var directory = stateDirectory ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "SuperDuper");
        _path = Path.Combine(Path.GetFullPath(directory), FileName);
    }

    public async Task<PresentationPreferences> LoadAsync(CancellationToken cancellationToken = default)
    {
        try
        {
            if (!File.Exists(_path) || new FileInfo(_path).Length > MaximumDocumentBytes)
            {
                return PresentationPreferences.Default;
            }

            await using var stream = new FileStream(
                _path,
                FileMode.Open,
                FileAccess.Read,
                FileShare.Read,
                bufferSize: 4096,
                FileOptions.Asynchronous | FileOptions.SequentialScan);
            var document = await JsonSerializer.DeserializeAsync<PersistedPreferences>(
                stream,
                SerializerOptions,
                cancellationToken).ConfigureAwait(false);
            return ToPreferences(document);
        }
        catch (Exception exception) when (
            exception is IOException
                or UnauthorizedAccessException
                or JsonException
                or NotSupportedException)
        {
            return PresentationPreferences.Default;
        }
    }

    public async Task SaveAsync(
        PresentationPreferences preferences,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(preferences);
        var document = ToPersisted(preferences);
        var bytes = JsonSerializer.SerializeToUtf8Bytes(document, SerializerOptions);
        if (bytes.Length > MaximumDocumentBytes)
        {
            throw new ArgumentOutOfRangeException(
                nameof(preferences),
                "Presentation preferences exceed the local storage limit.");
        }

        await _writeGate.WaitAsync(cancellationToken).ConfigureAwait(false);
        var temporaryPath = _path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            var directory = Path.GetDirectoryName(_path)
                ?? throw new IOException("Presentation preferences need a state directory.");
            Directory.CreateDirectory(directory);

            await using (var stream = new FileStream(
                temporaryPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 4096,
                FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                await stream.WriteAsync(bytes, cancellationToken).ConfigureAwait(false);
                await stream.FlushAsync(cancellationToken).ConfigureAwait(false);
            }

            File.Move(temporaryPath, _path, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporaryPath))
            {
                File.Delete(temporaryPath);
            }
            _writeGate.Release();
        }
    }

    private static PresentationPreferences ToPreferences(PersistedPreferences? document)
    {
        if (document is null || document.Version != PresentationPreferences.CurrentVersion)
        {
            return PresentationPreferences.Default;
        }

        if (!Enum.IsDefined(document.LastResultsMode)
            || document.SectionExpansion is null
            || document.SectionExpansion.Count > 128
            || document.SectionExpansion.Any(entry => string.IsNullOrWhiteSpace(entry.Key)
                || entry.Key.Length > 128))
        {
            return PresentationPreferences.Default;
        }

        var sections = new Dictionary<string, bool>(StringComparer.Ordinal);
        foreach (var entry in document.SectionExpansion)
        {
            sections[entry.Key] = entry.Value;
        }

        return new PresentationPreferences
        {
            IsSavedScanSelectorExpanded = document.IsSavedScanSelectorExpanded,
            LastResultsMode = document.LastResultsMode,
            SectionExpansion = new ReadOnlyDictionary<string, bool>(sections),
        };
    }

    private static PersistedPreferences ToPersisted(PresentationPreferences preferences)
    {
        if (!Enum.IsDefined(preferences.LastResultsMode)
            || preferences.SectionExpansion.Count > 128
            || preferences.SectionExpansion.Any(entry => string.IsNullOrWhiteSpace(entry.Key)
                || entry.Key.Length > 128))
        {
            throw new ArgumentOutOfRangeException(nameof(preferences), "Presentation preferences are invalid.");
        }

        return new PersistedPreferences
        {
            Version = PresentationPreferences.CurrentVersion,
            IsSavedScanSelectorExpanded = preferences.IsSavedScanSelectorExpanded,
            LastResultsMode = preferences.LastResultsMode,
            SectionExpansion = new Dictionary<string, bool>(preferences.SectionExpansion, StringComparer.Ordinal),
        };
    }

    private sealed class PersistedPreferences
    {
        public int Version { get; init; }

        public bool IsSavedScanSelectorExpanded { get; init; }

        public ResultsDisplayMode LastResultsMode { get; init; } = ResultsDisplayMode.Files;

        public Dictionary<string, bool>? SectionExpansion { get; init; } = [];
    }
}
