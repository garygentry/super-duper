namespace SuperDuper.Windows.Core.Workers;

public sealed record WorkerHelloResult(
    int ProtocolVersion,
    string WorkerVersion,
    string EngineVersion);
