using Windows.Storage.Pickers;

namespace SuperDuper.Services.Platform.Windows;

/// <summary>
/// Implements IFilePickerService using Windows Storage Pickers.
/// Moved out of code-behind to keep ViewModels platform-agnostic.
/// </summary>
public class WindowsFilePickerService : IFilePickerService
{
    public async Task<string?> PickFolderAsync()
    {
        var picker = new FolderPicker();
        InitPicker(picker);
        picker.SuggestedStartLocation = PickerLocationId.ComputerFolder;
        picker.FileTypeFilter.Add("*");

        var folder = await picker.PickSingleFolderAsync();
        return folder?.Path;
    }

    public async Task<string?> PickSaveFileAsync(
        string suggestedName,
        string fileTypeDescription = "CSV Files",
        string fileTypeExtension = ".csv")
    {
        var picker = new FileSavePicker();
        InitPicker(picker);
        picker.SuggestedFileName = suggestedName;
        picker.FileTypeChoices.Add(fileTypeDescription, new List<string> { fileTypeExtension });

        var file = await picker.PickSaveFileAsync();
        return file?.Path;
    }

    private static void InitPicker(object picker)
    {
        var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(App.MainWindow!);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
    }
}
