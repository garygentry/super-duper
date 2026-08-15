using CommunityToolkit.Mvvm.ComponentModel;
using SuperDuper.Windows.Core.Workers;

namespace SuperDuper.Windows.Core.ViewModels;

public sealed class ShellViewModel : ObservableObject
{
    private readonly IWorkerClient _workerClient;
    private WorkerConnectionState _connectionState = WorkerConnectionState.Starting;
    private string _statusTitle = "Starting worker";
    private string _statusDetail = "Establishing a private connection to the Super Duper engine.";
    private string? _workerVersion;
    private string? _engineVersion;

    public ShellViewModel(IWorkerClient workerClient)
    {
        _workerClient = workerClient;
    }

    public WorkerConnectionState ConnectionState
    {
        get => _connectionState;
        private set
        {
            if (SetProperty(ref _connectionState, value))
            {
                OnPropertyChanged(nameof(IsStarting));
                OnPropertyChanged(nameof(IsConnected));
                OnPropertyChanged(nameof(IsFailed));
            }
        }
    }

    public string StatusTitle
    {
        get => _statusTitle;
        private set => SetProperty(ref _statusTitle, value);
    }

    public string StatusDetail
    {
        get => _statusDetail;
        private set => SetProperty(ref _statusDetail, value);
    }

    public string? WorkerVersion
    {
        get => _workerVersion;
        private set => SetProperty(ref _workerVersion, value);
    }

    public string? EngineVersion
    {
        get => _engineVersion;
        private set => SetProperty(ref _engineVersion, value);
    }

    public string WorkerExecutablePath => _workerClient.ExecutablePath;

    public bool IsStarting => ConnectionState == WorkerConnectionState.Starting;

    public bool IsConnected => ConnectionState == WorkerConnectionState.Connected;

    public bool IsFailed => ConnectionState == WorkerConnectionState.Failed;

    public async Task InitializeAsync(CancellationToken cancellationToken = default)
    {
        ConnectionState = WorkerConnectionState.Starting;
        StatusTitle = "Starting worker";
        StatusDetail = "Establishing a private connection to the Super Duper engine.";
        WorkerVersion = null;
        EngineVersion = null;

        try
        {
            var hello = await _workerClient.ConnectAsync(cancellationToken).ConfigureAwait(true);

            WorkerVersion = hello.WorkerVersion;
            EngineVersion = hello.EngineVersion;
            StatusTitle = "Worker connected";
            StatusDetail = $"Protocol {hello.ProtocolVersion} · Worker {hello.WorkerVersion} · Engine {hello.EngineVersion}";
            ConnectionState = WorkerConnectionState.Connected;
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception exception)
        {
            StatusTitle = "Worker connection failed";
            StatusDetail = exception.Message;
            ConnectionState = WorkerConnectionState.Failed;
        }
    }
}
