using System.Diagnostics;
using System.Text.Json;
using SuperDuper.Windows.Infrastructure.Protocol;

namespace SuperDuper.Windows.Infrastructure.Tests;

[TestClass]
public sealed class WorkerRecoveryTests
{
    [TestMethod]
    public async Task KilledWorker_ReconcilesActiveRunAsInterruptedAfterRestart()
    {
        var worker = FindWorker();
        var temp = Path.Combine(Path.GetTempPath(), $"super-duper-recovery-test-{Guid.NewGuid():N}");
        var root = Path.Combine(temp, "root");
        var database = Path.Combine(temp, "worker.db");
        Directory.CreateDirectory(root);
        for (var index = 0; index < 1_500; index++)
        {
            await File.WriteAllBytesAsync(
                Path.Combine(root, $"{index:D4}.bin"),
                new byte[4096]);
        }

        try
        {
            using (var process = StartWorker(worker, database))
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
                    new { name = "Recovery", roots = new[] { root }, ignorePatterns = Array.Empty<string>() });
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
                database);
            _ = await restarted.ConnectAsync();
            var durable = await restarted.GetRunAsync(1);

            Assert.AreEqual("interrupted", durable.Status);
            Assert.IsNotNull(durable.CompletedAt);
        }
        finally
        {
            if (Directory.Exists(temp))
            {
                Directory.Delete(temp, recursive: true);
            }
        }
    }

    private static Process StartWorker(string executable, string database)
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
        var process = Process.Start(startInfo) ?? throw new InvalidOperationException("Worker did not start.");
        process.ErrorDataReceived += static (_, _) => { };
        process.BeginErrorReadLine();
        return process;
    }

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
