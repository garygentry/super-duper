using System.ComponentModel;
using System.Windows;
using System.Windows.Threading;
using System.Windows.Controls;
using System.Windows.Automation;
using SuperDuper.Windows.Accessibility;
using SuperDuper.Windows.Core.Services;
using SuperDuper.Windows.Core.ViewModels;
using SuperDuper.Windows.Core.Workers;
using SuperDuper.Windows.Infrastructure;

namespace SuperDuper.Windows;

public partial class MainWindow : Window
{
    public static readonly DependencyProperty IsSavedScanPaneOpenProperty = DependencyProperty.Register(
        nameof(IsSavedScanPaneOpen), typeof(bool), typeof(MainWindow), new PropertyMetadata(false,
            (sender, args) => ((MainWindow)sender).OnSavedScanPaneChanged((bool)args.NewValue)));

    public bool IsSavedScanPaneOpen
    {
        get => (bool)GetValue(IsSavedScanPaneOpenProperty);
        set => SetValue(IsSavedScanPaneOpenProperty, value);
    }

    internal static DispatcherPriority ShutdownDispatcherPriority => DispatcherPriority.Normal;

    private readonly IWorkerClient _workerClient;
    private readonly CancellationTokenSource _lifetime = new();
    private Task _initialization = Task.CompletedTask;
    private bool _shutdownStarted;
    private bool _shutdownComplete;
    private long _focusNavigationGeneration;
    private readonly IPresentationPreferencesStore? _preferencesStore;
    private PresentationPreferences _preferences = PresentationPreferences.Default;
    private bool _restoringPreferences;
    private readonly HashSet<Expander> _trackedSections = [];
    private Task _preferenceWrite = Task.CompletedTask;
    private bool _wasCompact;

    public MainWindow(ShellViewModel viewModel, IWorkerClient workerClient)
        : this(viewModel, workerClient, ownsWorkerLifetime: true) { }

    public MainWindow(ShellViewModel viewModel, IWorkerClient workerClient, IPresentationPreferencesStore preferences)
        : this(viewModel, workerClient, ownsWorkerLifetime: true, preferencesStore: preferences) { }

    // The isolated presentation fixture supplies fake services and manages their lifetime.
    internal MainWindow(ShellViewModel viewModel, IWorkerClient workerClient, bool ownsWorkerLifetime,
        ITextScaleSource? textScaleSource = null, IPresentationPreferencesStore? preferencesStore = null)
    {
        InitializeComponent();
        var textScale = new WindowTextScale(this, textScaleSource ?? new WindowsTextScaleSource());
        Closed += (_, _) => textScale.Dispose();
        ViewModel = viewModel;
        _workerClient = workerClient;
        _preferencesStore = preferencesStore;
        DataContext = viewModel;
        UpdatePrimaryActionStyle();
        ViewModel.CanNavigateOnCompletion = () => !System.Windows.Interop.ComponentDispatcher.IsThreadModal
            && IsEnabled && !OwnedWindows.Cast<Window>().Any(window => window.IsVisible);
        SizeChanged += (_, _) => UpdateResponsiveSelector();
        var fontDescriptor = DependencyPropertyDescriptor.FromProperty(FontSizeProperty, typeof(MainWindow));
        EventHandler fontChanged = (_, _) => UpdateResponsiveSelector();
        fontDescriptor.AddValueChanged(this, fontChanged);
        Closed += (_, _) => fontDescriptor.RemoveValueChanged(this, fontChanged);
        Loaded += (_, _) => TrackDisclosureSections(this);
        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        Closed += (_, _) => ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
        if (ownsWorkerLifetime) Closing += OnClosing;
    }

    public ShellViewModel ViewModel { get; }

    private void UpdatePrimaryActionStyle() => StartScanButton.SetResourceReference(StyleProperty,
        ViewModel.SelectedDestination is WorkspaceDestination.ScanSetup or WorkspaceDestination.ScanSummary
            ? "ShellPrimaryAction" : "ShellAction");

    private void UpdateResponsiveSelector()
    {
        var compact = ActualWidth < 1100 || FontSize > 18;
        if (compact == _wasCompact) return;
        var restoring = _restoringPreferences;
        _restoringPreferences = true;
        IsSavedScanPaneOpen = !compact && _preferences.IsSavedScanSelectorExpanded;
        _restoringPreferences = restoring;
        _wasCompact = compact;
    }

    public bool IsShutdownRequested => _shutdownStarted;

    public Task InitializeAsync()
    {
        _initialization = InitializeWithPreferencesAsync();
        return _initialization;
    }

    private async Task InitializeWithPreferencesAsync()
    {
        if (_preferencesStore is not null)
        {
            _preferences = await _preferencesStore.LoadAsync(_lifetime.Token);
            _restoringPreferences = true;
            IsSavedScanPaneOpen = ActualWidth >= 1100 && FontSize <= 18 && _preferences.IsSavedScanSelectorExpanded;
            ViewModel.ResultsDestination = _preferences.LastResultsMode == ResultsDisplayMode.Folders
                ? WorkspaceDestination.FolderResults : WorkspaceDestination.FileResults;
            foreach (var section in _trackedSections) RestoreSection(section);
            _restoringPreferences = false;
        }
        await ViewModel.InitializeAsync(_lifetime.Token);
    }

    private void OnSavedScanPaneChanged(bool expanded)
    {
        if (_restoringPreferences || _preferencesStore is null) return;
        _preferences = _preferences with { IsSavedScanSelectorExpanded = expanded };
        SavePreferences();
    }

    private static string? SectionKey(Expander section)
    {
        var id = AutomationProperties.GetAutomationId(section);
        if (string.IsNullOrEmpty(id)) id = section.Name;
        if (string.IsNullOrEmpty(id)) return null;
        var owner = Window.GetWindow(section);
        return owner is null ? null : id;
    }

    private void TrackDisclosureSections(DependencyObject parent)
    {
        if (_preferencesStore is null) return;
        // Loaded is a direct event, so a window handler cannot observe descendant loads.
        // These sections are declared in the logical tree, including inactive tab content.
        if (parent is Expander section && SectionKey(section) is not null && _trackedSections.Add(section))
        {
            RestoreSection(section);
            section.Expanded += OnSectionExpansionChanged;
            section.Collapsed += OnSectionExpansionChanged;
        }
        foreach (var child in LogicalTreeHelper.GetChildren(parent).OfType<DependencyObject>())
            TrackDisclosureSections(child);
    }

    private void RestoreSection(Expander section)
    {
        if (SectionKey(section) is { } key && _preferences.SectionExpansion.TryGetValue(key, out var expanded))
            section.SetCurrentValue(Expander.IsExpandedProperty, expanded);
    }

    private void OnSectionExpansionChanged(object sender, RoutedEventArgs args)
    {
        if (_restoringPreferences || args.OriginalSource is not Expander section || SectionKey(section) is not { } key) return;
        _preferences = _preferences.WithSectionExpanded(key, section.IsExpanded);
        SavePreferences();
    }

    private void SavePreferences()
    {
        if (_preferencesStore is null || _restoringPreferences) return;
        var snapshot = _preferences;
        var previous = _preferenceWrite;
        _preferenceWrite = PersistAsync();
        async Task PersistAsync()
        {
            await previous;
            try { await _preferencesStore.SaveAsync(snapshot); }
            catch (Exception error) when (error is System.IO.IOException or UnauthorizedAccessException)
            { System.Diagnostics.Trace.TraceWarning("Presentation preferences could not be saved: {0}", error.Message); }
        }
    }

    private void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(ShellViewModel.ResultsDestination) && !_restoringPreferences)
        {
            var mode = ViewModel.ResultsDestination == WorkspaceDestination.FolderResults ? ResultsDisplayMode.Folders : ResultsDisplayMode.Files;
            if (_preferences.LastResultsMode != mode)
            {
                _preferences = _preferences with { LastResultsMode = mode };
                SavePreferences();
            }
        }
        if (e.PropertyName == nameof(ShellViewModel.HasSetupDeparture))
        {
            _ = Dispatcher.BeginInvoke(() =>
            {
                if (ViewModel.HasSetupDeparture) StayInSetupButton.Focus();
                else (ScanTabs.SelectedItem as System.Windows.Controls.TabItem)?.Focus();
            }, DispatcherPriority.Background);
        }
        if (e.PropertyName == nameof(ShellViewModel.SelectedDestination))
        {
            _focusNavigationGeneration++;
            UpdatePrimaryActionStyle();
        }
        if (e.PropertyName != nameof(ShellViewModel.FocusRequestVersion))
        {
            return;
        }
        var version = ViewModel.FocusRequestVersion;
        var target = ViewModel.FocusTarget;
        var destination = ViewModel.SelectedDestination;
        var navigation = _focusNavigationGeneration;
        bool IsCurrent() => IsLoaded && ViewModel.FocusRequestVersion == version
            && ViewModel.SelectedDestination == destination && _focusNavigationGeneration == navigation;
        _ = Dispatcher.BeginInvoke(() =>
        {
            if (!IsCurrent()) return;
            if (target == "start-scan") StartScanButton.Focus();
            else if (target is "results-navigation" or "scan-navigation")
            {
                var tabs = target == "results-navigation" ? ResultsTabs : ScanTabs;
                (tabs.SelectedItem as System.Windows.Controls.TabItem)?.Focus();
            }
            else if (target == "duplicate-file-groups")
                _ = DuplicateFilesWorkspace.RestoreGroupGridFocusAsync(IsCurrent);
            else if (target == "duplicate-folder-groups")
                _ = DuplicateFoldersWorkspace.RestoreGroupGridFocusAsync();
            else if (target == "progress-warnings")
                _ = ProgressWorkspace.RestoreWarningEntryFocus();
            else if (target == "summary-warnings")
                _ = SummaryWorkspace.RestoreWarningEntryFocus();
            else if (target == "performance-heading")
                _ = PerformanceWorkspace.RestoreHeadingFocus();
            else if (target == "history-performance")
                _ = HistoryWorkspace.RestorePerformanceEntryFocus();
            else if (target == "history-grid")
                _ = HistoryWorkspace.RestoreHistoryGridFocusAsync();
            else if (target == "progress-performance")
                _ = ProgressWorkspace.RestorePerformanceEntryFocus();
            else if (target == "summary-performance")
                _ = SummaryWorkspace.RestorePerformanceEntryFocus();
        }, DispatcherPriority.Background);
    }

    private void OnClosing(object? sender, CancelEventArgs e)
    {
        if (_shutdownComplete)
        {
            return;
        }

        e.Cancel = true;
        if (_shutdownStarted)
        {
            return;
        }

        _shutdownStarted = true;
        _ = ShutdownAsync();
    }

    private async Task ShutdownAsync()
    {
        if (!await ViewModel.ConfirmCancelAndExitAsync())
        {
            _shutdownStarted = false;
            return;
        }

        try
        {
            IsEnabled = false;
            _lifetime.Cancel();
            try
            {
                await _initialization;
            }
            catch (OperationCanceledException) when (_lifetime.IsCancellationRequested)
            {
            }
            await _workerClient.DisposeAsync();
            await _preferenceWrite;
        }
        catch (Exception exception)
        {
            MessageBox.Show(
                this,
                $"Super Duper could not shut down its owned worker safely.\n\n{exception.Message}",
                "Shutdown failed",
                MessageBoxButton.OK,
                MessageBoxImage.Error);
            IsEnabled = true;
            _shutdownStarted = false;
            return;
        }

        _ = Dispatcher.BeginInvoke(
            () =>
            {
                _shutdownComplete = true;
                Application.Current.Shutdown();
            },
            ShutdownDispatcherPriority);
    }
}
