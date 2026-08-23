using System.Text.Json;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;

return await AmbiguousStartHost.RunAsync(args);

internal static class AmbiguousStartHost
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    public static async Task<int> RunAsync(string[] args)
    {
        try
        {
            var options = ParseArguments(args);
            var mode = Required(options, "mode");
            return mode switch
            {
                "prepare" => await PrepareAsync(options),
                "verify" => await VerifyAsync(options),
                _ => throw new ArgumentException("--mode must be prepare or verify."),
            };
        }
        catch (Exception exception)
        {
            Console.Error.WriteLine(exception);
            return 1;
        }
    }

    private static async Task<int> PrepareAsync(IReadOnlyDictionary<string, string> options)
    {
        var workerPath = Path.GetFullPath(Required(options, "worker"));
        var databasePath = Path.GetFullPath(Required(options, "database"));
        var hashCachePath = Path.GetFullPath(Required(options, "hash-cache"));
        var fixtureRoot = Path.GetFullPath(Required(options, "fixture-root"));
        var evidenceRoot = Path.GetFullPath(Required(options, "evidence-root"));
        Directory.CreateDirectory(fixtureRoot);
        Directory.CreateDirectory(evidenceRoot);

        Environment.SetEnvironmentVariable("SUPER_DUPER_DB_PATH", databasePath);
        Environment.SetEnvironmentVariable("HASH_CACHE_PATH", hashCachePath);

        var token = Guid.NewGuid().ToString("N");
        var paths = Enumerable.Range(1, 3)
            .Select(index => Path.Combine(fixtureRoot, $"WPM11-ambiguous-source-{index}-{token}.bin"))
            .ToArray();
        var payload = $"Super Duper WPM11 ambiguous-start disposable fixture {token}";
        foreach (var path in paths)
        {
            await File.WriteAllTextAsync(path, payload);
        }

        await using var client = new WorkerClient(workerPath);
        var terminal = new TaskCompletionSource<string>(TaskCreationOptions.RunContinuationsAsynchronously);
        client.RunLifecycleChanged += (_, eventArgs) =>
        {
            if (eventArgs.EventName is "run.completed" or "run.cancelled" or "run.failed")
            {
                terminal.TrySetResult(eventArgs.EventName);
            }
        };

        _ = await client.ConnectAsync();
        var session = await client.CreateSessionAsync("WPM11 ambiguous-start campaign", [fixtureRoot], []);
        var started = await client.StartRunAsync(session.Id);
        var terminalEvent = await terminal.Task.WaitAsync(TimeSpan.FromMinutes(2));
        var run = await client.GetRunAsync(started.Id);
        Require(terminalEvent == "run.completed" && run.Status == "completed", "Disposable scan did not complete.");

        var groups = await client.GetDuplicateFileGroupsAsync(new DuplicateFileGroupQuery(
            run.Id,
            20,
            DuplicateFileGroupSortField.RecoverableBytes,
            WorkerSortDirection.Descending,
            new DuplicateFileGroupFilter(string.Empty, "0")));
        Require(groups.Groups.Count == 1, $"Expected one disposable duplicate group, found {groups.Groups.Count}.");
        var group = groups.Groups.Single();
        var members = await client.GetDuplicateFileGroupMembersAsync(new DuplicateFileMemberQuery(
            run.Id,
            group.Id,
            20,
            DuplicateFileMemberSortField.Path,
            WorkerSortDirection.Ascending,
            new DuplicateFileMemberFilter(string.Empty)));
        Require(members.Members.Count == 3, $"Expected three disposable members, found {members.Members.Count}.");

        var plan = await client.GetReviewPlanAsync(run.Id);
        var ordered = members.Members.OrderBy(member => member.Path, StringComparer.Ordinal).ToArray();
        var revision = plan.Plan.Revision;
        var keep = await client.SetReviewDecisionAsync(
            $"campaign-keep-{token}", run.Id, group.Id, ordered[0].Id, "keep", revision);
        revision = keep.AppliedRevision;
        foreach (var member in ordered.Skip(1))
        {
            var remove = await client.SetReviewDecisionAsync(
                $"campaign-remove-{member.Id}-{token}", run.Id, group.Id, member.Id, "remove", revision);
            revision = remove.AppliedRevision;
        }

        var preflightStart = await client.StartPreflightAsync(
            $"campaign-preflight-{token}", run.Id, revision);
        var preflight = await WaitForPreflightAsync(client, preflightStart.Preflight.Id);
        Require(preflight.Status == "completed" && preflight.ReadyCount == 3 && preflight.TotalItemCount == 3,
            $"Expected a completed three-item survivor/removal preflight, got {preflight.Status} with {preflight.ReadyCount}/{preflight.TotalItemCount} ready.");

        var prepared = await client.PrepareRecycleOperationAsync(
            $"campaign-operation-{token}", run.Id, preflight.Id, revision);
        Require(!prepared.ExecutorEnabled, "Worker unexpectedly reported executorEnabled:true during prepare.");
        var operationItems = await client.GetRecycleOperationItemsAsync(
            new RecycleOperationItemQuery(prepared.Operation.Id, 20));
        Require(operationItems.Items.Count == 2, $"Expected two operation items, found {operationItems.Items.Count}.");

        using var executor = new WindowsRecycleOperationExecutor();
        var eligibility = await executor.InspectAsync(operationItems.Items);
        Require(eligibility.All(item => item.Status == "eligible"), "Disposable fixed-drive items were not Recycle Bin eligible.");
        var eligible = await client.ReportRecycleEligibilityAsync(
            $"campaign-eligibility-{token}", prepared.Operation.Id, eligibility);
        Require(!eligible.ExecutorEnabled, "Worker unexpectedly reported executorEnabled:true during eligibility.");
        Require(eligible.Operation.Status == "awaiting_confirmation", "Operation did not reach awaiting_confirmation.");
        Require(!string.IsNullOrWhiteSpace(eligible.Operation.ConfirmationSignature), "Worker did not issue a confirmation signature.");

        var confirmed = await client.ConfirmRecycleOperationAsync(
            $"campaign-confirm-{token}", prepared.Operation.Id, eligible.Operation.ConfirmationSignature!);
        Require(!confirmed.ExecutorEnabled, "Worker unexpectedly reported executorEnabled:true during confirmation.");
        Require(confirmed.Operation.Status == "submitted", "Operation did not reach submitted.");

        var next = await client.GetNextRecycleOperationBatchAsync(prepared.Operation.Id);
        Require(!next.ExecutorEnabled, "Worker unexpectedly reported executorEnabled:true during admission.");
        var batch = next.Batch ?? throw new InvalidOperationException("Worker returned no admitted batch.");
        Require(batch.Status == "admitted" && batch.Items.Count == 2, "Worker did not return the expected admitted batch.");

        var description = new
        {
            schemaVersion = 1,
            gate = "WPM11-ambiguous-start",
            disposable = true,
            hostProcessId = Environment.ProcessId,
            workerPath,
            databasePath,
            hashCachePath,
            fixtureRoot,
            fixtureToken = token,
            fixturePaths = paths,
            payloadLength = payload.Length,
            sessionId = session.Id,
            runId = run.Id,
            reviewRevision = revision,
            preflightId = preflight.Id,
            recycleOperationId = prepared.Operation.Id,
            batchId = batch.Id,
            itemIds = batch.Items.Select(item => item.Id).ToArray(),
            shellAttemptId = $"campaign-shell-{token}",
            boundary = "The host blocks inside the durable acknowledgement callback before IFileOperation.PerformOperations. Only this host is intentionally terminated.",
        };
        await WriteJsonAsync(Path.Combine(evidenceRoot, "fixture-description.json"), description);

        await executor.ExecuteBatchAsync(
            batch,
            async cancellationToken =>
            {
                var begun = await client.BeginRecycleOperationBatchAsync(
                    $"campaign-begin-{token}",
                    prepared.Operation.Id,
                    batch.Id,
                    description.shellAttemptId,
                    cancellationToken);
                Require(!begun.ExecutorEnabled, "Worker unexpectedly reported executorEnabled:true during durable begin.");
                Require(begun.Operation.Status == "executing", "Durable begin did not put the operation in executing.");
                await WriteJsonAsync(Path.Combine(evidenceRoot, "durable-shell-start.json"), new
                {
                    schemaVersion = 1,
                    recordedAtUtc = DateTimeOffset.UtcNow,
                    hostProcessId = Environment.ProcessId,
                    recycleOperationId = begun.Operation.Id,
                    operationStatus = begun.Operation.Status,
                    batchId = batch.Id,
                    itemIds = batch.Items.Select(item => item.Id).ToArray(),
                    shellAttemptId = description.shellAttemptId,
                    executorEnabled = begun.ExecutorEnabled,
                    performOperationsCalled = false,
                });
                await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
            });

        throw new InvalidOperationException("Disposable host unexpectedly returned from the durable-start hold.");
    }

    private static async Task<int> VerifyAsync(IReadOnlyDictionary<string, string> options)
    {
        var workerPath = Path.GetFullPath(Required(options, "worker"));
        var databasePath = Path.GetFullPath(Required(options, "database"));
        var hashCachePath = Path.GetFullPath(Required(options, "hash-cache"));
        var outputPath = Path.GetFullPath(Required(options, "output"));
        var operationId = long.Parse(Required(options, "operation-id"));
        Environment.SetEnvironmentVariable("SUPER_DUPER_DB_PATH", databasePath);
        Environment.SetEnvironmentVariable("HASH_CACHE_PATH", hashCachePath);

        await using var client = new WorkerClient(workerPath);
        _ = await client.ConnectAsync();
        var operation = await client.GetRecycleOperationAsync(operationId);
        var items = await client.GetRecycleOperationItemsAsync(
            new RecycleOperationItemQuery(operationId, 200, "unknown"));
        var review = await client.GetRecoveryReviewAsync(operationId);
        var history = await client.GetRecoveryReviewObservationsAsync(
            new RecoveryReviewObservationQuery(operationId, 200, false));
        var current = await client.GetRecoveryReviewObservationsAsync(
            new RecoveryReviewObservationQuery(operationId, 200, true));
        await WriteJsonAsync(outputPath, new
        {
            schemaVersion = 1,
            capturedAtUtc = DateTimeOffset.UtcNow,
            operation,
            unknownItems = items,
            review,
            history,
            current,
            executorEnabled = new
            {
                review = review.ExecutorEnabled,
                history = history.ExecutorEnabled,
                current = current.ExecutorEnabled,
            },
        });
        return 0;
    }

    private static async Task<WorkerPreflight> WaitForPreflightAsync(WorkerClient client, long preflightId)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromMinutes(2));
        while (true)
        {
            var preflight = await client.GetPreflightAsync(preflightId, timeout.Token);
            if (preflight.Status is "completed" or "cancelled" or "failed" or "interrupted")
            {
                return preflight;
            }
            await Task.Delay(100, timeout.Token);
        }
    }

    private static Dictionary<string, string> ParseArguments(string[] args)
    {
        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        for (var index = 0; index < args.Length; index += 2)
        {
            if (index + 1 >= args.Length || !args[index].StartsWith("--", StringComparison.Ordinal))
            {
                throw new ArgumentException("Arguments must be supplied as --name value pairs.");
            }
            values.Add(args[index][2..], args[index + 1]);
        }
        return values;
    }

    private static string Required(IReadOnlyDictionary<string, string> options, string name) =>
        options.TryGetValue(name, out var value) && !string.IsNullOrWhiteSpace(value)
            ? value
            : throw new ArgumentException($"Missing --{name}.");

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private static Task WriteJsonAsync(string path, object value)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        return File.WriteAllTextAsync(path, JsonSerializer.Serialize(value, JsonOptions));
    }
}
