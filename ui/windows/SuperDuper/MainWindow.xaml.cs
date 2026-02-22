using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;
using SuperDuper.Services;
using SuperDuper.ViewModels;
using SuperDuper.Views;

namespace SuperDuper;

public sealed partial class MainWindow : Window
{
    public ShellViewModel ViewModel { get; }
    private readonly IDatabaseService _db;
    private DispatcherTimer? _searchDebounce;
    private bool _isClosing;

    // NavigationView and children are built in code-behind because the raw XAML
    // parser (DisableXbfGeneration=true) doesn't wire NavigationView's internal
    // click-to-selection routing. API-created controls work correctly.
    private NavigationView NavView = null!;
    private Grid AppTitleBar = null!;
    private AutoSuggestBox SearchBox = null!;
    private Frame ContentFrame = null!;

    public MainWindow()
    {
        this.InitializeComponent();
        this.Closed += MainWindow_Closed;
        XamlHelper.ConnectNamedElements(this, (FrameworkElement)Content);
        Title = "Super Duper";

        ViewModel = App.Services.GetRequiredService<ShellViewModel>();
        _db = App.Services.GetRequiredService<IDatabaseService>();

        BuildNavigationView();

        // Set DataContext on root element so {Binding} can find ViewModel
        if (Content is FrameworkElement root)
        {
            root.DataContext = this;
            RegisterAccelerators(root, ShortcutDefinitions.Undo, OnUndoAccelerator);
            RegisterAccelerators(root, ShortcutDefinitions.Redo, OnRedoAccelerator);
            RegisterAccelerators(root, ShortcutDefinitions.OpenDeletionDialog, OnOpenDeletionDialogAccelerator);
            RegisterAccelerators(root, ShortcutDefinitions.RefreshCurrentView, OnRefreshAccelerator);
        }

        // Wire status bar button events (XAML elements wired by XamlHelper)
        ReviewDeleteButton.Click += ReviewDeleteButton_Click;

        SystemBackdrop = new MicaBackdrop { Kind = MicaKind.Base };
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        // Set a reasonable default window size (spec: 1024x768 minimum)
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1280, 860));

        // Surface navigation errors visibly
        ContentFrame.NavigationFailed += (_, e) =>
        {
            System.Diagnostics.Debug.WriteLine($"NAV FAILED: {e.SourcePageType?.Name} — {e.Exception}");
            e.Handled = true;
        };

        // Set initial page to Dashboard
        NavView.SelectedItem = NavView.MenuItems[0];
        ContentFrame.Navigate(typeof(DashboardPage));

        // Subscribe to deletion dialog requests from ShellViewModel commands
        ShellViewModel.OpenDeletionDialogRequested += OnOpenDeletionDialogRequested;

        // Refresh deletion count on load
        _ = ViewModel.RefreshDeletionCountAsync();

        // Set window icon
        var iconPath = System.IO.Path.Combine(
            System.IO.Path.GetDirectoryName(
                System.Reflection.Assembly.GetExecutingAssembly().Location)!,
            "Assets", "AppIcon.ico");
        if (System.IO.File.Exists(iconPath))
            AppWindow.SetIcon(iconPath);
    }

    /// <summary>
    /// Builds the NavigationView and all its children entirely in code-behind.
    /// This is necessary because WinUI 3's raw XAML parser (used when
    /// DisableXbfGeneration=true) doesn't correctly wire NavigationView's
    /// internal click-to-selection routing, making user clicks non-functional.
    /// API-created NavigationView works correctly.
    /// </summary>
    private void BuildNavigationView()
    {
        NavView = new NavigationView
        {
            IsBackButtonVisible = NavigationViewBackButtonVisible.Collapsed,
            IsSettingsVisible = true,
            PaneDisplayMode = NavigationViewPaneDisplayMode.LeftCompact,
            IsTitleBarAutoPaddingEnabled = false,
        };
        AutomationProperties.SetName(NavView, "Main navigation");

        // Pane header with app title (also used as custom title bar)
        AppTitleBar = new Grid { Height = 48 };
        var titleText = new TextBlock
        {
            Text = "Super Duper",
            VerticalAlignment = VerticalAlignment.Center,
            Margin = new Thickness(12, 0, 0, 0),
        };
        if (Application.Current.Resources.TryGetValue("BodyStrongTextBlockStyle", out var titleStyle))
            titleText.Style = (Style)titleStyle;
        AppTitleBar.Children.Add(titleText);
        NavView.PaneHeader = AppTitleBar;

        // Auto-suggest search box
        SearchBox = new AutoSuggestBox
        {
            PlaceholderText = "Search files and paths...",
            QueryIcon = new SymbolIcon(Symbol.Find),
        };
        AutomationProperties.SetName(SearchBox, "Global file search");
        SearchBox.TextChanged += SearchBox_TextChanged;
        SearchBox.QuerySubmitted += SearchBox_QuerySubmitted;
        NavView.AutoSuggestBox = SearchBox;

        // Menu items
        NavView.MenuItems.Add(CreateNavItem("Dashboard", "dashboard", Symbol.Home,
            "Dashboard — scan overview"));
        NavView.MenuItems.Add(CreateNavItem("Explorer", "explorer", Symbol.BrowsePhotos,
            "Explorer — browse and review duplicate files"));
        NavView.MenuItems.Add(CreateNavItem("Groups", "groups", Symbol.Copy,
            "Groups — all duplicate groups"));
        NavView.MenuItems.Add(CreateNavItem("Directories", "directories", Symbol.Folder,
            "Directories — similar directory pairs"));

        // Content frame
        ContentFrame = new Frame();
        NavView.Content = ContentFrame;

        // Wire navigation events
        NavView.SelectionChanged += NavView_SelectionChanged;

        // Insert NavigationView into the root Grid at Row 0
        Grid.SetRow(NavView, 0);
        ((Grid)Content).Children.Insert(0, NavView);
    }

    private static NavigationViewItem CreateNavItem(string content, string tag, Symbol icon, string automationName)
    {
        var item = new NavigationViewItem
        {
            Content = content,
            Tag = tag,
            Icon = new SymbolIcon(icon),
        };
        AutomationProperties.SetName(item, automationName);
        return item;
    }

    private void NavigateToPage(string tag)
    {
        var pageType = tag switch
        {
            "dashboard" => typeof(DashboardPage),
            "explorer" => typeof(ExplorerPage),
            "groups" => typeof(GroupsPage),
            "directories" => typeof(DirectoriesPage),
            "settings" => typeof(SettingsPage),
            _ => (Type?)null
        };
        if (pageType != null && ContentFrame.CurrentSourcePageType != pageType)
        {
            ContentFrame.Navigate(pageType);
        }
    }

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.IsSettingsSelected)
        {
            NavigateToPage("settings");
        }
        else if (args.SelectedItemContainer is NavigationViewItem item)
        {
            var tag = item.Tag?.ToString();
            if (tag != null) NavigateToPage(tag);
        }
    }

    private async void ReviewDeleteButton_Click(object sender, RoutedEventArgs e)
    {
        await OpenDeletionDialogAsync();
    }

    private async void OnOpenDeletionDialogRequested(object? sender, EventArgs e)
    {
        await OpenDeletionDialogAsync();
    }

    private async Task OpenDeletionDialogAsync()
    {
        var dialog = new DeletionConfirmationDialog
        {
            XamlRoot = this.Content.XamlRoot
        };
        await dialog.ShowAsync();
        await ViewModel.RefreshDeletionCountAsync();
    }

    private async void OnUndoAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        await ViewModel.UndoCommand.ExecuteAsync(null);
    }

    private async void OnRedoAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        await ViewModel.RedoCommand.ExecuteAsync(null);
    }

    private void OnOpenDeletionDialogAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        ReviewDeleteButton_Click(sender, new RoutedEventArgs());
    }

    private void OnRefreshAccelerator(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        if (ContentFrame.CurrentSourcePageType != null)
            ContentFrame.Navigate(ContentFrame.CurrentSourcePageType);
    }

    private void SearchBox_TextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput) return;

        _searchDebounce?.Stop();
        _searchDebounce = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(300) };
        _searchDebounce.Tick += async (_, _) =>
        {
            _searchDebounce.Stop();
            var query = sender.Text?.Trim();
            if (string.IsNullOrEmpty(query) || query.Length < 2)
            {
                sender.ItemsSource = null;
                return;
            }

            var sessionId = App.Services.GetRequiredService<ScanService>().ActiveSessionId ?? 0;
            var results = await _db.SearchFilesAsync(sessionId, query);
            sender.ItemsSource = results.Select(f => f.DisplayPath).ToList();
        };
        _searchDebounce.Start();
    }

    private static void RegisterAccelerators(UIElement target, ShortcutDefinitions.ShortcutEntry entry,
        Windows.Foundation.TypedEventHandler<KeyboardAccelerator, KeyboardAcceleratorInvokedEventArgs> handler)
    {
        foreach (var binding in entry.Bindings)
        {
            var accel = new KeyboardAccelerator { Key = binding.Key, Modifiers = binding.Modifiers };
            accel.Invoked += handler;
            target.KeyboardAccelerators.Add(accel);
        }
    }

    private async void MainWindow_Closed(object sender, WindowEventArgs args)
    {
        if (_isClosing) return;
        _isClosing = true;

        // Defer the close so we can await scan cancellation before disposing
        args.Handled = true;

        var scanService = App.Services.GetRequiredService<ScanService>();
        await scanService.CancelAndWaitAsync();

        (App.Services as IDisposable)?.Dispose();
        this.Close();
    }

    private void SearchBox_QuerySubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        if (args.ChosenSuggestion is string path)
        {
            var parentDir = System.IO.Path.GetDirectoryName(path);
            ContentFrame.Navigate(typeof(ExplorerPage), parentDir);
        }
        else if (!string.IsNullOrWhiteSpace(args.QueryText))
        {
            ContentFrame.Navigate(typeof(GroupsPage), args.QueryText);
        }
    }
}
