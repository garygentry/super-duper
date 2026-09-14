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
    private readonly Func<long, string?> _sessionName;
    private readonly Action<PerformanceReturnDestination>? _returnFromPerformance;
    private CancellationTokenSource? _loadCancellation;
    private WorkerRun? _productRun;
    private WorkerPerformanceSnapshot? _current;
    private PerformanceRunListItemViewModel? _selectedComparisonRun;
    private PerformanceDeviceItemViewModel? _selectedDevice;
    private PerformanceReturnDestination _returnDestination;
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

    public PerformanceViewModel(
        IWorkerClient workerClient,
        Func<long, string?>? sessionName = null,
        Action<PerformanceReturnDestination>? returnFromPerformance = null)
    {
        _workerClient = workerClient;
        _sessionName = sessionName ?? (_ => null);
        _returnFromPerformance = returnFromPerformance;
        RefreshCommand = new AsyncRelayCommand(() => RefreshAsync(), () => HasRun && !IsBusy);
        CompareCommand = new AsyncRelayCommand(CompareAsync, () => HasRun && !IsBusy && SelectedComparisonRun is not null);
        ReturnCommand = new RelayCommand(ReturnFromPerformance, () => HasRun && _returnFromPerformance is not null);
    }

    public ObservableCollection<PerformanceRunListItemViewModel> History { get; } = [];

    public ObservableCollection<PerformancePhaseItemViewModel> Phases { get; } = [];

    public ObservableCollection<PerformanceDeviceItemViewModel> Devices { get; } = [];

    public bool HasRun => _productRun is not null;

    public long? ProductRunId => _productRun?.Id;

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

    public PerformanceDeviceItemViewModel? SelectedDevice
    {
        get => _selectedDevice;
        set
        {
            if (SetProperty(ref _selectedDevice, value))
            {
                OnPropertyChanged(nameof(HasSelectedDevice));
                OnPropertyChanged(nameof(SelectedDeviceHeading));
            }
        }
    }

    public bool HasSelectedDevice => SelectedDevice is not null;

    public string SelectedDeviceHeading => SelectedDevice is { } device
        ? $"{device.Device} · {device.Volume}"
        : "No drive selected";

    public string ContextHeading => _returnDestination switch
    {
        PerformanceReturnDestination.ScanProgress => "Performance · active scan",
        PerformanceReturnDestination.ScanSummary => "Performance · scan summary",
        PerformanceReturnDestination.Workspace => "Performance · opened scan",
        _ => "Performance · highlighted scan",
    };

    public string ContextIdentity => _productRun is { } run
        ? $"{_sessionName(run.SessionId) ?? "Saved scan"} · Scan {run.Id:N0} · "
          + $"{(run.StartedAt ?? run.CreatedAt).ToLocalTime():g} · {DisplayFormatting.Status(run.Status)}"
        : "No scan selected";

    public string SnapshotBoundary => _current is { } current && _productRun is { } run
        ? $"Worker telemetry {current.Run.Id:N0} for exact Scan {run.Id:N0} · metrics contract v{current.Run.MetricsContractVersion}. "
          + "Current and peak values are persisted summaries; raw samples and time-series data are not available."
        : "Loading the worker-owned bounded summary for this exact scan. Raw samples and time-series data are not available.";

    public string ReturnLabel => _returnDestination switch
    {
        PerformanceReturnDestination.ScanProgress => "_Return to progress",
        PerformanceReturnDestination.ScanSummary => "_Return to scan summary",
        _ => "_Return to scan history",
    };

    public string ReturnAutomationName => _returnDestination switch
    {
        PerformanceReturnDestination.ScanProgress => "Close performance details and return focus to the active scan performance entry",
        PerformanceReturnDestination.ScanSummary => "Close performance details and return focus to the scan summary performance entry",
        PerformanceReturnDestination.Workspace => "Close performance details and return focus to the opened scan in history",
        _ => "Close performance details and return focus to the highlighted scan performance entry",
    };

    public string RunStatus => _current is null ? "Unavailable" : DisplayFormatting.Status(_current.Run.State);
    public string RunDuration => _current is null ? "—" : Duration(_current.Run.LastMonotonicNanos);
    public string CandidateFunnel => _current is null ? "—" : Funnel(_current);
    public string CacheSummary => _current is null ? "—" : Cache(_current);
    public string ReadSummary => _current is null ? "—" : Reads(_current);
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
    public IRelayCommand ReturnCommand { get; }

    public async Task ShowRunAsync(
        WorkerRun? run,
        CancellationToken cancellationToken = default,
        PerformanceReturnDestination returnDestination = PerformanceReturnDestination.History)
    {
        var runChanged = _productRun?.Id != run?.Id || _productRun?.SessionId != run?.SessionId;
        _productRun = run;
        _returnDestination = returnDestination;
        OnPropertyChanged(nameof(HasRun));
        OnPropertyChanged(nameof(ProductRunId));
        NotifyContextChanged();
        _nextLiveRefreshSequence = 0;
        ResetComparison();
        ReturnCommand.NotifyCanExecuteChanged();
        if (runChanged)
        {
            _current = null;
            History.Clear();
            Phases.Clear();
            Devices.Clear();
            SelectedDevice = null;
            NotifySummaryChanged();
        }
        if (run is null)
        {
            CancelLoad();
            _current = null;
            History.Clear();
            Phases.Clear();
            Devices.Clear();
            SelectedDevice = null;
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
            History.Clear();
            Phases.Clear();
            Devices.Clear();
            SelectedComparisonRun = null;
            SelectedDevice = null;
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
        if (_productRun is not { } productRun || snapshot.Run.ProductRunId != productRun.Id)
        {
            throw new InvalidDataException("The worker returned performance telemetry for a different product run.");
        }
        if (page.Runs.Select(run => run.Id).Distinct().Count() != page.Runs.Count)
        {
            throw new InvalidDataException("The worker returned duplicate performance history rows.");
        }
        var selectedDeviceKey = SelectedDevice?.IdentityKey;
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
        SelectedDevice = selectedDeviceKey is null
            ? Devices.FirstOrDefault()
            : Devices.FirstOrDefault(device => device.IdentityKey == selectedDeviceKey) ?? Devices.FirstOrDefault();
        NotifySummaryChanged();
        NotifyContextChanged();
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
            if (comparison.Run.Id != selectedStatusRunId
                || comparison.Run.ProductRunId != selected.ProductRunId)
            {
                throw new InvalidDataException("The worker returned comparison telemetry for a different selected run.");
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
            var comparisonIdentity = comparison.Run.ProductRunId is long comparisonProductRunId
                ? $"Scan {comparisonProductRunId:N0} · telemetry {comparison.Run.Id:N0}"
                : $"Telemetry {comparison.Run.Id:N0} without a recorded scan link";
            ComparisonMessage = differences.Count == 0
                ? $"Comparable with {comparisonIdentity}: same volume/device, scan inputs, and software build."
                : $"Context differs from {comparisonIdentity}: {string.Join(", ", differences)}. Values are shown but are not a like-for-like result.";
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
        OnPropertyChanged(nameof(ReadSummary));
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
        var partial = CacheStage(snapshot, "partial_hash", "Partial");
        var full = CacheStage(snapshot, "full_hash", "Full");
        return $"{partial} · {full}";
    }

    private static string CacheStage(WorkerPerformanceSnapshot snapshot, string prefix, string label)
    {
        if (!TryCounter(snapshot, $"{prefix}_cache_hits", out var hits))
        {
            return $"{label}: unavailable (no counter summary recorded)";
        }
        var misses = Counter(snapshot, $"{prefix}_cache_misses");
        var errors = Counter(snapshot, $"{prefix}_cache_errors");
        var stores = Counter(snapshot, $"{prefix}_cache_stores");
        var total = hits + misses + errors;
        return total == 0
            ? $"{label}: unavailable (no cache lookups recorded)"
            : $"{label}: {hits * 100m / total:0.0}% hits · {hits:N0} hit / {misses:N0} miss / {errors:N0} error / {stores:N0} store";
    }

    private static string Reads(WorkerPerformanceSnapshot snapshot)
    {
        var logical = TryCounter(snapshot, "candidate_bytes", out var candidateBytes)
            ? $"{DisplayFormatting.Bytes(candidateBytes.ToString(CultureInfo.InvariantCulture))} logical candidate data"
            : "Logical candidate data unavailable";
        var partial = TryCounter(snapshot, "partial_hash_bytes_read", out var partialBytes)
            ? $"{DisplayFormatting.Bytes(partialBytes.ToString(CultureInfo.InvariantCulture))} partial bytes actually read"
            : "Partial actual reads unavailable";
        var full = TryCounter(snapshot, "full_hash_bytes_read", out var fullBytes)
            ? $"{DisplayFormatting.Bytes(fullBytes.ToString(CultureInfo.InvariantCulture))} full bytes actually read"
            : "Full actual reads unavailable";
        return $"{logical} · {partial} · {full}. Logical work is not disk throughput.";
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

    private void ReturnFromPerformance()
    {
        if (_productRun is null || _returnFromPerformance is null) return;
        _returnFromPerformance(_returnDestination);
    }

    private void NotifyContextChanged()
    {
        OnPropertyChanged(nameof(ContextHeading));
        OnPropertyChanged(nameof(ContextIdentity));
        OnPropertyChanged(nameof(SnapshotBoundary));
        OnPropertyChanged(nameof(ReturnLabel));
        OnPropertyChanged(nameof(ReturnAutomationName));
    }

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
    public long? ProductRunId => run.ProductRunId;
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
    public string IdentityKey => $"{device.Descriptor.DeviceKey}\u001f{device.Descriptor.VolumeKey}";
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

public enum PerformanceReturnDestination
{
    History,
    ScanProgress,
    ScanSummary,
    Workspace,
}
