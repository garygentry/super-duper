using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Converters;
using SuperDuper.NativeMethods;
using SuperDuper.Services;
using System.Globalization;
using System.Text;

namespace SuperDuper.Views;

public sealed partial class DeletionConfirmationDialog : ContentDialog
{
    private readonly IDatabaseService _db;
    private readonly EngineWrapper _engine;
    private readonly DispatcherQueue _dispatcher;
    private IReadOnlyList<FileForDeletion> _queue = Array.Empty<FileForDeletion>();
    private DispatcherTimer? _countdownTimer;
    private int _countdown = 3;

    public DeletionConfirmationDialog()
    {
        this.InitializeComponent();
        _db = App.Services.GetRequiredService<IDatabaseService>();
        _engine = App.Services.GetRequiredService<EngineWrapper>();
        _dispatcher = DispatcherQueue.GetForCurrentThread();

        Opened += OnOpened;
        PrimaryButtonClick += OnPrimaryButtonClick;
    }

    private async void OnOpened(ContentDialog sender, ContentDialogOpenedEventArgs args)
    {
        _queue = await _db.GetDeletionQueueAsync();

        var totalBytes = _queue.Sum(f => f.FileSize);
        var driveCount = _queue.Select(f => f.DriveLetter).Distinct().Count();
        SummaryText.Text = $"{_queue.Count:N0} files · {FileSizeConverter.FormatBytes(totalBytes)} · across {driveCount} drive(s)";

        // Check for network paths
        var hasNetwork = _queue.Any(f => f.CanonicalPath.StartsWith(@"\\"));
        NetworkDriveWarning.IsOpen = hasNetwork;

        // Start countdown
        StartCountdown();
    }

    private void StartCountdown()
    {
        _countdown = 3;
        IsPrimaryButtonEnabled = false;
        UpdateCountdownButton();

        _countdownTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        _countdownTimer.Tick += (_, _) =>
        {
            _countdown--;
            if (_countdown <= 0)
            {
                _countdownTimer.Stop();
                IsPrimaryButtonEnabled = _queue.Count > 0;
                PrimaryButtonText = $"Delete {_queue.Count:N0} Files";
            }
            else
            {
                UpdateCountdownButton();
            }
        };
        _countdownTimer.Start();
    }

    private void UpdateCountdownButton()
    {
        PrimaryButtonText = $"Delete in {_countdown}...";
    }

    private async void OnPrimaryButtonClick(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        // Defer to allow the dialog to stay open during deletion
        var deferral = args.GetDeferral();

        try
        {
            var useTrash = GetSelectedDeletionMode() == "trash";
            DeletionProgress.Visibility = Visibility.Visible;

            await Task.Run(() => _engine.ExecuteDeletionPlan(useTrash));

            DeletionProgress.Value = 100;
            await WriteCsvLogAsync(_queue);

            SuccessBar.Message = $"Deleted {_queue.Count:N0} files. Deletion log saved to Documents.";
            SuccessBar.IsOpen = true;

            // Keep dialog open briefly to show success
            await Task.Delay(1500);
        }
        catch (Exception ex)
        {
            SuccessBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
            SuccessBar.Title = "Deletion failed";
            SuccessBar.Message = ex.Message;
            SuccessBar.IsOpen = true;
        }
        finally
        {
            deferral.Complete();
        }
    }

    private string GetSelectedDeletionMode()
    {
        foreach (var item in DeletionModeRadio.Items)
        {
            if (item is RadioButton rb && rb.IsChecked == true)
                return rb.Tag?.ToString() ?? "trash";
        }
        return "trash";
    }

    private async Task WriteCsvLogAsync(IReadOnlyList<FileForDeletion> files)
    {
        try
        {
            var docsPath = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
            var logDir = Path.Combine(docsPath, "Super Duper");
            Directory.CreateDirectory(logDir);
            var fileName = $"Super Duper_DeletionLog_{DateTime.Now:yyyyMMdd_HHmmss}.csv";
            var logPath = Path.Combine(logDir, fileName);

            var sb = new StringBuilder();
            sb.AppendLine("original_path,content_hash,size_bytes,retained_copy_path,deleted_at,method");
            var method = GetSelectedDeletionMode();
            var now = DateTime.UtcNow.ToString("O", CultureInfo.InvariantCulture);
            foreach (var f in files)
            {
                var hash = ((ulong)f.ContentHash).ToString("X16");
                sb.AppendLine($"\"{f.CanonicalPath}\",{hash},{f.FileSize},\"{f.RetainedCopyPath ?? ""}\",{now},{method}");
            }
            await File.WriteAllTextAsync(logPath, sb.ToString());
        }
        catch { /* Log write failure is non-critical */ }
    }
}
