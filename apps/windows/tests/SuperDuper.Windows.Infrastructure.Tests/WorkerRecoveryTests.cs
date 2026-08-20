using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;
using Microsoft.Win32.SafeHandles;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerRecoveryTests
{
    [TestMethod]
    public async Task KilledOwnedWorker_RaisesTypedExitAndSameClientRestartsForNewRun()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-owned-recovery-{Guid.NewGuid():N}");
        var root = Path.Combine(temp, "root");
        WorkerClient? client = null;
        int? restartedProcessId = null;
        Directory.CreateDirectory(root);
        for (var index = 0; index < 1_500; index++)
        {
            await File.WriteAllBytesAsync(Path.Combine(root, $"{index:D4}.bin"), new byte[4096]);
        }
        CreateSparseRecoveryDelay(root);

        try
        {
            client = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                Path.Combine(temp, "worker.db"),
                Path.Combine(temp, "logs", "worker.log"),
                Path.Combine(temp, "hash-cache"));
            var unexpected = new TaskCompletionSource<WorkerUnexpectedExitEventArgs>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            client.UnexpectedExit += (_, exit) => unexpected.TrySetResult(exit);

            var hello = await client.ConnectAsync();
            var session = await client.CreateSessionAsync("Owned recovery", [root], []);
            var abandoned = await client.StartRunAsync(session.Id);
            var processId = client.OwnedProcessId;
            Assert.IsNotNull(processId);

            using (var process = Process.GetProcessById(processId.Value))
            {
                process.Kill(entireProcessTree: true);
                await process.WaitForExitAsync();
            }

            var exit = await unexpected.Task.WaitAsync(TimeSpan.FromSeconds(10));
            Assert.AreNotEqual(0, exit.ExitCode);
            StringAssert.Contains(exit.Message, "unexpectedly");

            var restartedHello = await client.RestartAsync();
            restartedProcessId = client.OwnedProcessId;
            var reconciled = await client.GetRunAsync(abandoned.Id);
            Assert.AreEqual(hello.ProtocolVersion, restartedHello.ProtocolVersion);
            Assert.AreEqual("interrupted", reconciled.Status);

            var terminal = new TaskCompletionSource<WorkerRun>(TaskCreationOptions.RunContinuationsAsynchronously);
            client.RunLifecycleChanged += (_, lifecycle) =>
            {
                if (lifecycle.Run.Status is "completed" or "cancelled" or "failed")
                {
                    terminal.TrySetResult(lifecycle.Run);
                }
            };
            var rerun = await client.StartRunAsync(session.Id);
            var completed = await terminal.Task.WaitAsync(TimeSpan.FromSeconds(30));
            Assert.AreEqual(rerun.Id, completed.Id);
            Assert.AreEqual("completed", completed.Status);
        }
        finally
        {
            if (client is not null)
            {
                await client.DisposeAsync();
            }
            if (restartedProcessId is int processId)
            {
                Assert.ThrowsException<ArgumentException>(() => Process.GetProcessById(processId));
            }
            if (Directory.Exists(temp))
            {
                await TestDirectoryCleanup.DeleteAsync(temp);
            }
        }
    }

    [TestMethod]
    public async Task KilledWorker_ReconcilesActiveRunAsInterruptedAfterRestart()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-recovery-test-{Guid.NewGuid():N}");
        var root = Path.Combine(temp, "root");
        var database = Path.Combine(temp, "worker.db");
        var hashCache = Path.Combine(temp, "hash-cache");
        Directory.CreateDirectory(root);
        for (var index = 0; index < 1_500; index++)
        {
            await File.WriteAllBytesAsync(
                Path.Combine(root, $"{index:D4}.bin"),
                new byte[4096]);
        }
        CreateSparseRecoveryDelay(root);

        try
        {
            using (var process = StartWorker(worker, database, hashCache))
            {
                await SendAsync(
                    process,
                    "hello",
                    "hello",
                    new
                    {
                        protocolVersions = new[] { 1 },
                        client = new { name = "recovery-test", version = "1.0.0" },
                    });
                _ = await ReadResponseAsync(process.StandardOutput, "hello");

                await SendAsync(
                    process,
                    "create",
                    "session.create",
                    new
                    {
                        name = "Recovery",
                        roots = new[] { root },
                        ignorePatterns = Array.Empty<string>(),
                        cloudPolicy = CloudPolicyNames.ExcludeRegisteredRoots,
                        manualLocationExclusions = Array.Empty<string>(),
                        registeredCloudLocations = Array.Empty<object>(),
                        cloudDetectionStatus = CloudDetectionStatusNames.Complete,
                    });
                _ = await ReadResponseAsync(process.StandardOutput, "create");

                await SendAsync(process, "start", "run.start", new { sessionId = 1 });
                var started = await ReadResponseAsync(process.StandardOutput, "start");
                Assert.AreEqual(
                    "running",
                    started.GetProperty("result").GetProperty("run").GetProperty("status").GetString());

                process.Kill(entireProcessTree: true);
                await process.WaitForExitAsync();
            }

            await using var restarted = new WorkerClient(
                worker,
                TimeSpan.FromSeconds(10),
                database,
                Path.Combine(temp, "logs", "restarted.log"),
                hashCache);
            _ = await restarted.ConnectAsync();
            var durable = await restarted.GetRunAsync(1);

            Assert.AreEqual("interrupted", durable.Status);
            Assert.IsNotNull(durable.CompletedAt);
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                await TestDirectoryCleanup.DeleteAsync(temp);
            }
        }
    }

    private static Process StartWorker(string executable, string database, string hashCache)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = executable,
            WorkingDirectory = Path.GetDirectoryName(executable)!,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        startInfo.Environment["SUPER_DUPER_DB_PATH"] = database;
        startInfo.Environment["HASH_CACHE_PATH"] = hashCache;
        var process = Process.Start(startInfo) ?? throw new InvalidOperationException("Worker did not start.");
        process.ErrorDataReceived += static (_, _) => { };
        process.BeginErrorReadLine();
        return process;
    }

    private static void CreateSparseRecoveryDelay(string root)
    {
        const uint fsctlSetSparse = 0x000900C4;
        const long logicalLength = 1024L * 1024 * 1024;
        var path = Path.Combine(root, "recovery-delay.sparse");
        using var stream = new FileStream(path, FileMode.CreateNew, FileAccess.ReadWrite, FileShare.Read);
        Assert.IsTrue(
            DeviceIoControl(
                stream.SafeFileHandle,
                fsctlSetSparse,
                nint.Zero,
                0,
                nint.Zero,
                0,
                out _,
                nint.Zero),
            new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()).Message);
        stream.SetLength(logicalLength);
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeviceIoControl(
        SafeFileHandle device,
        uint ioControlCode,
        nint inputBuffer,
        uint inputBufferSize,
        nint outputBuffer,
        uint outputBufferSize,
        out uint bytesReturned,
        nint overlapped);

    private static async Task SendAsync(Process process, string id, string method, object parameters)
    {
        await process.StandardInput.WriteAsync(JsonLineProtocol.EncodeRequestFrame(id, method, parameters));
        await process.StandardInput.FlushAsync();
    }

    private static async Task<JsonElement> ReadResponseAsync(StreamReader reader, string id)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(10));
        while (await reader.ReadLineAsync(timeout.Token) is { } line)
        {
            using var document = JsonDocument.Parse(line);
            var root = document.RootElement;
            if (root.GetProperty("type").GetString() == "response" &&
                root.GetProperty("id").GetString() == id)
            {
                Assert.IsTrue(root.GetProperty("ok").GetBoolean(), line);
                return root.Clone();
            }
        }

        Assert.Fail($"Worker exited before response {id}.");
        return default;
    }

    private static string FindWorker()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            var candidate = Path.Combine(directory.FullName, "target", BuildProfile, "super-duper-worker.exe");
            if (File.Exists(candidate))
            {
                return candidate;
            }
        }

        Assert.Inconclusive("Build the Rust workspace before running Windows integration tests.");
        return string.Empty;
    }

#if DEBUG
    private const string BuildProfile = "debug";
#else
    private const string BuildProfile = "release";
#endif
}
