using System.Collections.ObjectModel;
using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class PerformanceViewModel : ObservableObject, IDisposable
{
    public const int HistoryLimit = 25;
    public const int DeviceLimit = 64;
    public const int PhaseLimit = 6;
    private const ulong LiveRefreshSequenceInterval = 50;

    private readonly IWorkerClient _workerClient;
    private CancellationTokenSource? _loadCancellation;
    private WorkerRun? _productRun;
    private WorkerPerformanceSnapshot? _current;
    private PerformanceRunListItemViewModel? _selectedComparisonRun;
    private bool _isBusy;
    private string _statusMessage = "Select a scan to view bounded performance telemetry.";
    private string? _errorMessage;
    private string _comparisonMessage = "Select a prior telemetry run to compare.";
    private string _comparisonDuration = "—";
    private string _comparisonThroughput = "—";
    private string _comparisonWarnings = "—";
    private string _comparisonPeakRead = "—";
    private long _generation;
    private ulong _nextLiveRefreshSequence;
    private long _announcementVersion;
    private bool _disposed;

    public PerformanceViewModel(IWorkerClient workerClient)
    {
        _workerClient = workerClient;
        RefreshCommand = new AsyncRelayCommand(() => RefreshAsync(), () => HasRun && !IsBusy);
        CompareCommand = new AsyncRelayCommand(CompareAsync, () => HasRun && !IsBusy && SelectedComparisonRun is not null);
    }

    public ObservableCollection<PerformanceRunListItemViewModel> History { get; } = [];

    public ObservableCollection<PerformancePhaseItemViewModel> Phases { get; } = [];

    public ObservableCollection<PerformanceDeviceItemViewModel> Devices { get; } = [];

    public bool HasRun => _productRun is not null;

    public bool IsBusy
    {
        get => _isBusy;
        private set
        {
            if (SetProperty(ref _isBusy, value))
            {
                RefreshCommand.NotifyCanExecuteChanged();
                CompareCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string StatusMessage
    {
        get => _statusMessage;
        private set => SetProperty(ref _statusMessage, value);
    }

    public string? ErrorMessage
    {
        get => _errorMessage;
        private set
        {
            if (SetProperty(ref _errorMessage, value))
            {
                OnPropertyChanged(nameof(HasError));
            }
        }
    }

    public bool HasError => !string.IsNullOrWhiteSpace(ErrorMessage);

    public long AnnouncementVersion
    {
        get => _announcementVersion;
        private set => SetProperty(ref _announcementVersion, value);
    }

    public PerformanceRunListItemViewModel? SelectedComparisonRun
    {
        get => _selectedComparisonRun;
        set
        {
            if (SetProperty(ref _selectedComparisonRun, value))
            {
                CompareCommand.NotifyCanExecuteChanged();
            }
        }
    }

    public string RunStatus => _current is null ? "Unavailable" : DisplayFormatting.Status(_current.Run.State);
    public string RunDuration => _current is null ? "—" : Duration(_current.Run.LastMonotonicNanos);
    public string CandidateFunnel => _current is null ? "—" : Funnel(_current);
    public string CacheSummary => _current is null ? "—" : Cache(_current);
    public string FullReadThroughput => _current is null ? "—" : Throughput(_current);
    public string CpuSummary => _current is null ? "Unavailable" : Cpu(_current.Host);
    public string MemorySummary => _current is null ? "Unavailable" : Memory(_current.Host);
    public string WarningSummary => _current is null || !TryCounter(_current, "warnings", out var warnings) ? "Unavailable" : warnings.ToString("N0");
    public string UnavailableSummary => _current is null ? "—" : Unavailable(_current);
    public string ComparisonMessage { get => _comparisonMessage; private set => SetProperty(ref _comparisonMessage, value); }
    public string ComparisonDuration { get => _comparisonDuration; private set => SetProperty(ref _comparisonDuration, value); }
    public string ComparisonThroughput { get => _comparisonThroughput; private set => SetProperty(ref _comparisonThroughput, value); }
    public string ComparisonWarnings { get => _comparisonWarnings; private set => SetProperty(ref _comparisonWarnings, value); }
    public string ComparisonPeakRead { get => _comparisonPeakRead; private set => SetProperty(ref _comparisonPeakRead, value); }
    public string CurrentPeakRead => _current is null ? "—" : PeakRead(_current);

    public IAsyncRelayCommand RefreshCommand { get; }
    public IAsyncRelayCommand CompareCommand { get; }

    public async Task ShowRunAsync(WorkerRun? run, CancellationToken cancellationToken = default)
    {
        _productRun = run;
        OnPropertyChanged(nameof(HasRun));
        _nextLiveRefreshSequence = 0;
        ResetComparison();
        if (run is null)
        {
            CancelLoad();
            _current = null;
            History.Clear();
            Phases.Clear();
            Devices.Clear();
            StatusMessage = "Select a scan to view bounded performance telemetry.";
            ErrorMessage = null;
            NotifySummaryChanged();
            RefreshCommand.NotifyCanExecuteChanged();
            CompareCommand.NotifyCanExecuteChanged();
            return;
        }
        await RefreshAsync(cancellationToken);
    }

    public void ObserveProgress(long runId, ulong sequence)
    {
        if (_disposed || _productRun?.Id != runId || sequence < _nextLiveRefreshSequence || IsBusy)
        {
            return;
        }
        _nextLiveRefreshSequence = sequence + LiveRefreshSequenceInterval;
        _ = RefreshAsync();
    }

    public void ObserveLifecycle(WorkerRun run)
    {
        if (_disposed || _productRun?.Id != run.Id)
        {
            return;
        }
        _productRun = run;
        _ = RefreshAsync();
    }

    public Task RefreshAsync() => RefreshAsync(CancellationToken.None);

    private async Task RefreshAsync(CancellationToken cancellationToken)
    {
        if (_productRun is not { } productRun || _disposed)
        {
            return;
        }
        CancelLoad();
        _loadCancellation = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var token = _loadCancellation.Token;
        var generation = ++_generation;
        IsBusy = true;
        ErrorMessage = null;
        StatusMessage = "Loading bounded performance history and current telemetry…";
        try
        {
            var historyTask = _workerClient.GetPerformanceRunsAsync(null, HistoryLimit, token);
            var snapshotTask = _workerClient.GetPerformanceSnapshotAsync(productRunId: productRun.Id, cancellationToken: token);
            await Task.WhenAll(historyTask, snapshotTask);
            token.ThrowIfCancellationRequested();
            if (generation != _generation)
            {
                return;
            }
            Apply(historyTask.Result, snapshotTask.Result);
            StatusMessage = $"Performance telemetry refreshed for run {productRun.Id}; {History.Count} bounded history rows and {Devices.Count} drive rows loaded.";
            AnnouncementVersion++;
        }
        catch (OperationCanceledException) when (token.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            if (generation != _generation)
            {
                return;
            }
            _current = null;
            Phases.Clear();
            Devices.Clear();
            ErrorMessage = $"Performance telemetry is unavailable: {exception.Message}";
            StatusMessage = "No performance values were substituted; unavailable fields remain unavailable.";
            AnnouncementVersion++;
            NotifySummaryChanged();
        }
        finally
        {
            if (generation == _generation)
            {
                IsBusy = false;
            }
        }
    }

    private void Apply(WorkerPerformanceRunPage page, WorkerPerformanceSnapshot snapshot)
    {
        if (page.ExecutorEnabled || snapshot.ExecutorEnabled)
        {
            throw new InvalidDataException("The performance query crossed the disabled production execution boundary.");
        }
        if (page.Runs.Count > HistoryLimit || snapshot.Phases.Count > PhaseLimit || snapshot.Devices.Count > DeviceLimit)
        {
            throw new InvalidDataException("The worker returned an unbounded performance collection.");
        }
        _current = snapshot;
        History.Clear();
        foreach (var run in page.Runs)
        {
            History.Add(new PerformanceRunListItemViewModel(run));
        }
        SelectedComparisonRun = History.FirstOrDefault(item => item.StatusRunId != snapshot.Run.Id);
        Phases.Clear();
        foreach (var phase in snapshot.Phases)
        {
            Phases.Add(new PerformancePhaseItemViewModel(phase));
        }
        Devices.Clear();
        foreach (var device in snapshot.Devices)
        {
            Devices.Add(new PerformanceDeviceItemViewModel(device));
        }
        NotifySummaryChanged();
    }

    private async Task CompareAsync()
    {
        if (_current is not { } current || SelectedComparisonRun is not { } selected)
        {
            return;
        }
        IsBusy = true;
        ErrorMessage = null;
        var generation = _generation;
        var selectedStatusRunId = selected.StatusRunId;
        try
        {
            var comparison = await _workerClient.GetPerformanceSnapshotAsync(statusRunId: selectedStatusRunId);
            if (generation != _generation || SelectedComparisonRun?.StatusRunId != selectedStatusRunId)
            {
                return;
            }
            if (comparison.ExecutorEnabled || comparison.Phases.Count > PhaseLimit || comparison.Devices.Count > DeviceLimit)
            {
                throw new InvalidDataException("The worker returned an unsafe or unbounded comparison snapshot.");
            }
            var differences = new List<string>();
            if (!DeviceIdentity(current).SequenceEqual(DeviceIdentity(comparison), StringComparer.Ordinal))
            {
                differences.Add("volume/device");
            }
            if (!string.Equals(current.Run.InputSignature, comparison.Run.InputSignature, StringComparison.Ordinal))
            {
                differences.Add("scan inputs");
            }
            if (!SameBuild(current.Run, comparison.Run))
            {
                differences.Add("software build");
            }
            ComparisonMessage = differences.Count == 0
                ? $"Comparable with telemetry run {comparison.Run.Id}: same volume/device, scan inputs, and software build."
                : $"Context differs from telemetry run {comparison.Run.Id}: {string.Join(", ", differences)}. Values are shown but are not a like-for-like result.";
            ComparisonDuration = Duration(comparison.Run.LastMonotonicNanos);
            ComparisonThroughput = Throughput(comparison);
            ComparisonWarnings = TryCounter(comparison, "warnings", out var warnings) ? warnings.ToString("N0") : "Unavailable";
            ComparisonPeakRead = PeakRead(comparison);
            StatusMessage = $"Loaded bounded comparison telemetry for run {comparison.Run.Id}.";
            AnnouncementVersion++;
        }
        catch (Exception exception)
        {
            if (generation != _generation)
            {
                return;
            }
            ErrorMessage = $"Comparison telemetry is unavailable: {exception.Message}";
            ComparisonMessage = "No comparison values were substituted.";
            ComparisonDuration = ComparisonThroughput = ComparisonWarnings = ComparisonPeakRead = "—";
            AnnouncementVersion++;
        }
        finally
        {
            if (generation == _generation)
            {
                IsBusy = false;
            }
        }
    }

    private void ResetComparison()
    {
        SelectedComparisonRun = null;
        ComparisonMessage = "Select a prior telemetry run to compare.";
        ComparisonDuration = ComparisonThroughput = ComparisonWarnings = ComparisonPeakRead = "—";
    }

    private void NotifySummaryChanged()
    {
        OnPropertyChanged(nameof(RunStatus));
        OnPropertyChanged(nameof(RunDuration));
        OnPropertyChanged(nameof(CandidateFunnel));
        OnPropertyChanged(nameof(CacheSummary));
        OnPropertyChanged(nameof(FullReadThroughput));
        OnPropertyChanged(nameof(CpuSummary));
        OnPropertyChanged(nameof(MemorySummary));
        OnPropertyChanged(nameof(WarningSummary));
        OnPropertyChanged(nameof(UnavailableSummary));
        OnPropertyChanged(nameof(CurrentPeakRead));
    }

    private static ulong Counter(WorkerPerformanceSnapshot snapshot, string name) =>
        snapshot.Counters.FirstOrDefault(counter => string.Equals(counter.Metric, name, StringComparison.Ordinal))?.Value ?? 0;

    private static bool TryCounter(WorkerPerformanceSnapshot snapshot, string name, out ulong value)
    {
        var counter = snapshot.Counters.FirstOrDefault(item => string.Equals(item.Metric, name, StringComparison.Ordinal));
        value = counter?.Value ?? 0;
        return counter is not null;
    }

    private static string Funnel(WorkerPerformanceSnapshot snapshot) => TryCounter(snapshot, "discovered_files", out _)
        ? $"{Counter(snapshot, "discovered_files"):N0} discovered → {Counter(snapshot, "metadata_resolved_files"):N0} metadata-only → {Counter(snapshot, "candidate_files"):N0} candidates → {Counter(snapshot, "partial_hashes_succeeded"):N0} partial → {Counter(snapshot, "full_hash_requests"):N0} full requests → {Counter(snapshot, "confirmed_physical_items"):N0} duplicate items"
        : "Unavailable (no counter summary recorded)";

    private static string Cache(WorkerPerformanceSnapshot snapshot)
    {
        if (!TryCounter(snapshot, "full_hash_cache_hits", out var hits))
        {
            return "Unavailable (no counter summary recorded)";
        }
        var misses = Counter(snapshot, "full_hash_cache_misses");
        var errors = Counter(snapshot, "full_hash_cache_errors");
        var total = hits + misses + errors;
        return total == 0 ? "Unavailable (no cache lookups recorded)" : $"{hits * 100m / total:0.0}% hits · {hits:N0} hit / {misses:N0} miss / {errors:N0} error";
    }

    private static string Throughput(WorkerPerformanceSnapshot snapshot)
    {
        var nanos = snapshot.Phases.FirstOrDefault(phase => phase.Phase == "full_hashing")?.ActiveNanos ?? 0;
        if (!TryCounter(snapshot, "full_hash_bytes_read", out var bytes))
        {
            return "Unavailable (no counter summary recorded)";
        }
        return nanos == 0 ? "Unavailable (no full-read duration recorded)" : $"{DisplayFormatting.Bytes(((decimal)bytes * 1_000_000_000m / nanos).ToString("0", CultureInfo.InvariantCulture))}/s";
    }

    private static string Cpu(WorkerHostPerformanceSummary host) => host.Latest?.SystemCpuBasisPoints is uint current
        ? $"System {current / 100m:0.##}% current; {(host.PeakSystemCpuBasisPoints is uint currentPeak ? $"{currentPeak / 100m:0.##}% peak" : "peak unavailable")}"
        : host.PeakSystemCpuBasisPoints is uint availablePeak ? $"Current unavailable; {availablePeak / 100m:0.##}% peak" : "Unavailable";

    private static string Memory(WorkerHostPerformanceSummary host) => host.Latest?.ProcessWorkingSetBytes is ulong current
        ? $"{DisplayFormatting.Bytes(current.ToString(CultureInfo.InvariantCulture))} current; {(host.PeakProcessWorkingSetBytes is ulong currentPeak ? DisplayFormatting.Bytes(currentPeak.ToString(CultureInfo.InvariantCulture)) + " peak" : "peak unavailable")}"
        : host.PeakProcessWorkingSetBytes is ulong availablePeak ? $"Current unavailable; {DisplayFormatting.Bytes(availablePeak.ToString(CultureInfo.InvariantCulture))} peak" : "Unavailable";

    private static string Unavailable(WorkerPerformanceSnapshot snapshot)
    {
        if (!TryCounter(snapshot, "unavailable_counters", out var cumulative))
        {
            return "Unavailable (no counter summary recorded)";
        }
        var latest = snapshot.Host.Latest?.UnavailableCounterCount;
        return latest is null ? $"{cumulative:N0} cumulative; current host counters unavailable" : $"{cumulative:N0} cumulative · {latest:N0} unavailable in latest host sample";
    }

    private static string PeakRead(WorkerPerformanceSnapshot snapshot)
    {
        var values = snapshot.Devices.Select(device => device.PeakReadBytesPerSecond).Where(value => value.HasValue).Select(value => value!.Value).ToArray();
        return values.Length == 0 ? "Unavailable" : $"{DisplayFormatting.Bytes(values.Max().ToString(CultureInfo.InvariantCulture))}/s";
    }

    private static string Duration(ulong nanos) => TimeSpan.FromTicks((long)Math.Min(nanos / 100, (ulong)long.MaxValue)).ToString(@"d\.hh\:mm\:ss", CultureInfo.InvariantCulture);

    private static IEnumerable<string> DeviceIdentity(WorkerPerformanceSnapshot snapshot) => snapshot.Devices
        .Select(device => $"{device.Descriptor.DeviceKey}\u001f{device.Descriptor.VolumeKey}")
        .Order(StringComparer.Ordinal);

    private static bool SameBuild(WorkerPerformanceRun left, WorkerPerformanceRun right) =>
        left.MetricsContractVersion == right.MetricsContractVersion
        && string.Equals(left.EngineVersion, right.EngineVersion, StringComparison.Ordinal)
        && string.Equals(left.WorkerVersion, right.WorkerVersion, StringComparison.Ordinal)
        && string.Equals(left.AppVersion, right.AppVersion, StringComparison.Ordinal)
        && left.ProductSchemaVersion == right.ProductSchemaVersion;

    private void CancelLoad()
    {
        _loadCancellation?.Cancel();
        _loadCancellation?.Dispose();
        _loadCancellation = null;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _generation++;
        CancelLoad();
    }
}

public sealed class PerformanceRunListItemViewModel(WorkerPerformanceRun run)
{
    public long StatusRunId => run.Id;
    public string Run => run.ProductRunId is long id ? $"Scan {id}" : $"Telemetry {run.Id}";
    public string Status => DisplayFormatting.Status(run.State);
    public string Started => run.StartedUnixMillis is long value ? DateTimeOffset.FromUnixTimeMilliseconds(value).ToLocalTime().ToString("g") : "Unavailable";
    public string Build => $"engine {run.EngineVersion} · worker {run.WorkerVersion ?? "unavailable"} · app {run.AppVersion ?? "unavailable"}";
}

public sealed class PerformancePhaseItemViewModel(WorkerPerformancePhase phase)
{
    public string Phase => DisplayFormatting.Phase(phase.Phase);
    public string State => DisplayFormatting.Status(phase.State);
    public string Duration => TimeSpan.FromTicks((long)Math.Min(phase.ActiveNanos / 100, (ulong)long.MaxValue)).ToString(@"hh\:mm\:ss\.fff", CultureInfo.InvariantCulture);
}

public sealed class PerformanceDeviceItemViewModel(WorkerDevicePerformanceSummary device)
{
    private static string Scaled(ulong? value, decimal divisor, string suffix) => value is ulong number ? $"{number / divisor:0.##} {suffix}" : "Unavailable";
    public string Device => device.Descriptor.Model ?? device.Descriptor.DeviceKey;
    public string Volume => device.Descriptor.VolumeKey;
    public string Details => string.Join(" · ", new[] { device.Descriptor.MediaType, device.Descriptor.BusType, device.Descriptor.Filesystem }.Where(value => !string.IsNullOrWhiteSpace(value))!);
    public string Capacity => device.Descriptor.CapacityBytes is ulong value ? DisplayFormatting.Bytes(value.ToString(CultureInfo.InvariantCulture)) : "Unavailable";
    public string FreeAtStart => device.Descriptor.FreeBytesAtStart is ulong value ? DisplayFormatting.Bytes(value.ToString(CultureInfo.InvariantCulture)) : "Unavailable";
    public string CurrentRead => device.Latest?.ReadBytesPerSecond is ulong value ? $"{DisplayFormatting.Bytes(value.ToString(CultureInfo.InvariantCulture))}/s" : "Unavailable";
    public string PeakRead => device.PeakReadBytesPerSecond is ulong value ? $"{DisplayFormatting.Bytes(value.ToString(CultureInfo.InvariantCulture))}/s" : "Unavailable";
    public string CurrentIops => Scaled(device.Latest?.ReadIopsMillis, 1000m, "IOPS");
    public string PeakIops => Scaled(device.PeakReadIopsMillis, 1000m, "IOPS");
    public string CurrentLatency => Scaled(device.Latest?.AverageReadLatencyMicros, 1000m, "ms");
    public string PeakLatency => Scaled(device.PeakAverageReadLatencyMicros, 1000m, "ms");
    public string CurrentActive => Scaled(device.Latest?.ActiveMillisPerSecond, 10m, "%");
    public string PeakActive => Scaled(device.PeakActiveMillisPerSecond, 10m, "%");
    public string CurrentQueue => Scaled(device.Latest?.QueueDepthMillis, 1000m, "depth");
    public string PeakQueue => Scaled(device.PeakQueueDepthMillis, 1000m, "depth");
    public string Availability => device.Latest is null ? "No device sample available" : $"{device.Latest.UnavailableCounterCount:N0} unavailable counters in latest sample";
}
