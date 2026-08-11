using CmlLib.Core.Auth.Microsoft;
using LemiCraft_Launcher.Utils;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using LemiCraft_Launcher.Models;

namespace LemiCraft_Launcher.Services
{
    public class AuthResult
    {
        public bool Success { get; set; }
        public string? ErrorMessage { get; set; }
        public UserProfile? Profile { get; set; }
    }

    public static class AuthService
    {
        private static readonly HttpClient _httpClient = new();

        private static readonly JsonSerializerOptions _writeOptions = new() { WriteIndented = true };

        private static readonly string UserDataDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LemiCraft");

        private static readonly string PlainProfilePath = Path.Combine(UserDataDir, "user.json");
        private static readonly string EncryptedProfilePath = Path.Combine(UserDataDir, "user.sec");

        public static async Task<AuthResult> LoginElyByAsync(string username, string password)
        {
            try
            {
                var clientToken = Guid.NewGuid().ToString("N");

                var payload = new { username, password, clientToken, requestUser = true };

                var json = JsonSerializer.Serialize(payload);
                var content = new StringContent(json, Encoding.UTF8, "application/json");

                var response = await _httpClient.PostAsync(
                    "https://authserver.ely.by/auth/authenticate",
                    content
                );

                if (!response.IsSuccessStatusCode)
                {
                    var errorJson = await response.Content.ReadAsStringAsync();

                    try
                    {
                        using var errorDoc = JsonDocument.Parse(errorJson);
                        var errorRoot = errorDoc.RootElement;

                        if (errorRoot.TryGetProperty("errorMessage", out var errorMsg))
                        {
                            return new AuthResult
                            {
                                Success = false,
                                ErrorMessage = errorMsg.GetString() ?? "Ошибка авторизации"
                            };
                        }
                    }
                    catch { }

                    return new AuthResult
                    {
                        Success = false,
                        ErrorMessage = $"Ошибка: {response.StatusCode}"
                    };
                }

                var responseJson = await response.Content.ReadAsStringAsync();
                using var responseDoc = JsonDocument.Parse(responseJson);
                var root = responseDoc.RootElement;

                var accessToken = root.GetProperty("accessToken").GetString() ?? "";
                var selectedProfile = root.GetProperty("selectedProfile");
                var uuid = selectedProfile.GetProperty("id").GetString() ?? "";
                var name = selectedProfile.GetProperty("name").GetString() ?? "";

                var profile = new UserProfile
                {
                    Username = name,
                    AccessToken = accessToken,
                    ClientToken = clientToken,
                    Uuid = uuid,
                    Provider = "Ely.by",
                    LastLogin = DateTime.Now
                };

                return new AuthResult
                {
                    Success = true,
                    Profile = profile
                };
            }
            catch (HttpRequestException ex)
            {
                return new AuthResult
                {
                    Success = false,
                    ErrorMessage = "Ошибка сети: " + ex.Message
                };
            }
            catch (Exception ex)
            {
                return new AuthResult
                {
                    Success = false,
                    ErrorMessage = "Неизвестная ошибка: " + ex.Message
                };
            }
        }

        public static async Task<bool?> ValidateElyByTokenAsync(string accessToken)
        {
            try
            {
                var payload = new { accessToken };
                var json = JsonSerializer.Serialize(payload);
                var content = new StringContent(json, Encoding.UTF8, "application/json");

                var response = await _httpClient.PostAsync(
                    "https://authserver.ely.by/auth/validate",
                    content
                );

                if (response.IsSuccessStatusCode) return true;

                if (response.StatusCode == HttpStatusCode.Unauthorized ||
                    response.StatusCode == HttpStatusCode.BadRequest)
                    return false;

                return null;
            }
            catch
            {
                return null;
            }
        }

        public static async Task<AuthResult> RefreshElyByTokenAsync(UserProfile profile)
        {
            try
            {
                var payload = new
                {
                    accessToken = profile.AccessToken,
                    clientToken = profile.ClientToken,
                    requestUser = true
                };

                var json = JsonSerializer.Serialize(payload);
                var content = new StringContent(json, Encoding.UTF8, "application/json");

                var response = await _httpClient.PostAsync(
                    "https://authserver.ely.by/auth/refresh",
                    content
                );

                if (!response.IsSuccessStatusCode)
                    return new AuthResult { Success = false, ErrorMessage = "Не удалось обновить токен" };

                var responseJson = await response.Content.ReadAsStringAsync();
                using var responseDoc = JsonDocument.Parse(responseJson);
                var root = responseDoc.RootElement;

                var newAccessToken = root.GetProperty("accessToken").GetString() ?? "";
                var selectedProfile = root.GetProperty("selectedProfile");
                var uuid = selectedProfile.GetProperty("id").GetString() ?? "";
                var name = selectedProfile.GetProperty("name").GetString() ?? "";

                var newProfile = new UserProfile
                {
                    Username = name,
                    AccessToken = newAccessToken,
                    ClientToken = profile.ClientToken,
                    Uuid = uuid,
                    Provider = "Ely.by",
                    LastLogin = DateTime.Now
                };

                return new AuthResult { Success = true, Profile = newProfile };
            }
            catch (Exception ex)
            {
                return new AuthResult { Success = false, ErrorMessage = ex.Message };
            }
        }

        public static void MigrateIfNeeded()
        {
            try
            {
                if (!File.Exists(EncryptedProfilePath) && File.Exists(PlainProfilePath))
                {
                    var json = File.ReadAllText(PlainProfilePath);
                    CryptoUtils.SaveEncryptedStringToFile(EncryptedProfilePath, json);

                    try
                    {
                        File.Delete(PlainProfilePath);
                    }
                    catch { }
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"MigrateIfNeeded error: {ex.Message}");
            }
        }

        public static void SaveProfile(UserProfile profile)
        {
            try
            {
                Directory.CreateDirectory(UserDataDir);
                var json = JsonSerializer.Serialize(profile, _writeOptions);
                CryptoUtils.SaveEncryptedStringToFile(EncryptedProfilePath, json);
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Ошибка сохранения профиля: {ex.Message}");
            }
        }

        public static UserProfile? LoadProfile()
        {
            try
            {
                var json = CryptoUtils.LoadEncryptedStringFromFile(EncryptedProfilePath);
                if (json != null)
                    return JsonSerializer.Deserialize<UserProfile>(json);

                if (File.Exists(PlainProfilePath))
                {
                    var plainJson = File.ReadAllText(PlainProfilePath);
                    return JsonSerializer.Deserialize<UserProfile>(plainJson);
                }

                return null;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"LoadProfile error: {ex.Message}");
                return null;
            }
        }

        public static void Logout()
        {
            try
            {
                if (File.Exists(EncryptedProfilePath)) File.Delete(EncryptedProfilePath);
                if (File.Exists(PlainProfilePath)) File.Delete(PlainProfilePath);
            }
            catch { }
        }

        public static async Task<AuthResult> AutoLoginAsync()
        {
            var profile = LoadProfile();
            if (profile == null)
                return new AuthResult { Success = false, ErrorMessage = "Нет сохраненного профиля" };

            if (profile.Provider == "Ely.by")
            {
                var validationResult = await ValidateElyByTokenAsync(profile.AccessToken);

                if (validationResult == true)
                {
                    profile.LastLogin = DateTime.Now;
                    SaveProfile(profile);
                    return new AuthResult { Success = true, Profile = profile };
                }

                if (validationResult == null)
                {
                    Debug.WriteLine("Сеть недоступна, используем кешированный профиль");
                    return new AuthResult { Success = true, Profile = profile };
                }

                var refreshResult = await RefreshElyByTokenAsync(profile);
                if (refreshResult.Success && refreshResult.Profile != null)
                {
                    SaveProfile(refreshResult.Profile);
                    return refreshResult;
                }

                Debug.WriteLine("Ely.by refresh failed, using cached profile");
                return new AuthResult { Success = true, Profile = profile };
            }
            if (profile.Provider == "Microsoft")
            {
                try
                {
                    var loginHandler = JELoginHandlerBuilder.BuildDefault();
                    var session = await loginHandler.Authenticate();
                    if (session != null && !string.IsNullOrWhiteSpace(session.AccessToken))
                    {
                        profile.Username = session.Username ?? "";
                        profile.AccessToken = session.AccessToken;
                        profile.Uuid = session.UUID ?? "";
                        profile.LastLogin = DateTime.Now;
                        SaveProfile(profile);

                        return new AuthResult { Success = true, Profile = profile };
                    }
                }
                catch (Exception ex)
                {
                    Debug.WriteLine("AutoLogin Microsoft error: " + ex);
                }

                return new AuthResult { Success = false, ErrorMessage = "Microsoft: silent login не удался" };
            }

            return new AuthResult { Success = false, ErrorMessage = "Сессия устарела. Войдите заново" };
        }

        /// <summary>
        /// Пытается определить, какой процесс занимает указанный TCP-порт (Windows, через netstat).
        /// Возвращает имя процесса или null, если определить не удалось.
        /// </summary>
        private static string? TryFindPortOwner(int port)
        {
            try
            {
                if (!OperatingSystem.IsWindows()) return null;

                var psi = new ProcessStartInfo
                {
                    FileName = "netstat",
                    Arguments = "-ano -p TCP",
                    RedirectStandardOutput = true,
                    UseShellExecute = false,
                    CreateNoWindow = true
                };

                using var proc = Process.Start(psi);
                if (proc == null) return null;

                var output = proc.StandardOutput.ReadToEnd();
                proc.WaitForExit(2000);

                foreach (var line in output.Split('\n'))
                {
                    var trimmed = line.Trim();
                    if (!trimmed.StartsWith("TCP", StringComparison.OrdinalIgnoreCase)) continue;
                    if (!trimmed.Contains($":{port} ") && !trimmed.Contains($":{port}\t")) continue;
                    if (!trimmed.Contains("LISTENING", StringComparison.OrdinalIgnoreCase)) continue;

                    var parts = trimmed.Split(' ', StringSplitOptions.RemoveEmptyEntries);
                    var localAddr = parts.Length > 1 ? parts[1] : "";
                    if (!localAddr.EndsWith($":{port}")) continue;

                    var pidStr = parts[^1];
                    if (!int.TryParse(pidStr, out var pid)) continue;

                    try
                    {
                        using var owner = Process.GetProcessById(pid);
                        return owner.ProcessName;
                    }
                    catch { return null; }
                }

                return null;
            }
            catch
            {
                return null;
            }
        }

        public static async Task<AuthResult> LoginElyByOAuthAsync(Action<string>? showMessageCallback = null)
        {
            var cfg = ConfigService.Load();
            var apiBaseUrl = cfg.ApiBaseUrl;

            try
            {
                var urlResponse = await _httpClient.GetAsync($"{apiBaseUrl}/auth/ely/url");
                if (!urlResponse.IsSuccessStatusCode)
                {
                    return new AuthResult { Success = false, ErrorMessage = "Не удалось получить OAuth URL с сервера" };
                }

                var urlJson = await urlResponse.Content.ReadAsStringAsync();
                using var urlDoc = JsonDocument.Parse(urlJson);
                var root = urlDoc.RootElement;

                var authUrl = root.GetProperty("authUrl").GetString() ?? "";
                var state = root.GetProperty("state").GetString() ?? "";
                var redirectUri = root.GetProperty("redirectUri").GetString() ?? "";

                if (!Uri.TryCreate(redirectUri, UriKind.Absolute, out var redirectUriParsed))
                {
                    return new AuthResult { Success = false, ErrorMessage = "Некорректный redirect URI" };
                }

                int port = redirectUriParsed.Port;

                using var listener = new HttpListener();
                listener.Prefixes.Add($"http://localhost:{port}/");
                try
                {
                    listener.Start();
                }
                catch
                {
                    var blocker = TryFindPortOwner(port);
                    var hint = blocker != null
                        ? $"Порт {port} занят приложением \"{blocker}\". Закройте его и попробуйте снова, либо перезагрузите компьютер"
                        : $"Порт {port} занят другим приложением на вашем компьютере. Закройте лишние программы (антивирус, другие лаунчеры, торрент-клиенты) или перезагрузите компьютер и попробуйте снова";

                    return new AuthResult { Success = false, ErrorMessage = $"Не удалось запустить локальный сервер для входа: {hint}" };
                }

                try
                {
                    Process.Start(new ProcessStartInfo { FileName = authUrl, UseShellExecute = true });
                }
                catch { }

                HttpListenerContext? context;
                var waitTask = Task.Run(() =>
                {
                    try
                    {
                        return listener.GetContext();
                    }
                    catch { return null; }
                });

                var timeout = Task.Delay(TimeSpan.FromMinutes(2));
                var finished = await Task.WhenAny(waitTask, timeout);
                if (finished == timeout)
                {
                    listener.Stop();
                    return new AuthResult { Success = false, ErrorMessage = "Таймаут ожидания авторизации" };
                }

                context = await waitTask;
                if (context == null)
                {
                    listener.Stop();
                    return new AuthResult { Success = false, ErrorMessage = "Ошибка получения ответа от сервера" };
                }

                var query = context.Request.QueryString;

                if (query["error"] != null)
                {
                    listener.Stop();
                    return new AuthResult { Success = false, ErrorMessage = query["error_message"] ?? "Ошибка авторизации" };
                }

                var returnedState = query["state"];
                var code = query["code"];

                if (returnedState != state)
                {
                    listener.Stop();
                    return new AuthResult { Success = false, ErrorMessage = "Некорректный state" };
                }

                var exchangePayload = new
                {
                    code,
                    state = returnedState
                };

                var exchangeJson = JsonSerializer.Serialize(exchangePayload);
                var exchangeContent = new StringContent(exchangeJson, Encoding.UTF8, "application/json");

                var exchangeResponse = await _httpClient.PostAsync($"{apiBaseUrl}/auth/ely/exchange", exchangeContent);

                if (!exchangeResponse.IsSuccessStatusCode)
                {
                    listener.Stop();
                    var errorText = await exchangeResponse.Content.ReadAsStringAsync();
                    return new AuthResult { Success = false, ErrorMessage = $"Ошибка обмена кода: {errorText}" };
                }

                var exchangeResponseJson = await exchangeResponse.Content.ReadAsStringAsync();
                using var exchangeDoc = JsonDocument.Parse(exchangeResponseJson);
                var exchangeRoot = exchangeDoc.RootElement;

                if (!exchangeRoot.TryGetProperty("success", out var successProp) || !successProp.GetBoolean())
                {
                    listener.Stop();
                    return new AuthResult { Success = false, ErrorMessage = "Ошибка обмена кода" };
                }

                var accessToken = exchangeRoot.GetProperty("accessToken").GetString() ?? "";
                var username = exchangeRoot.GetProperty("username").GetString() ?? "";
                var uuid = exchangeRoot.GetProperty("uuid").GetString() ?? "";

                var profile = new UserProfile
                {
                    Username = username,
                    AccessToken = accessToken,
                    ClientToken = Guid.NewGuid().ToString("N"),
                    Uuid = uuid,
                    Provider = "Ely.by",
                    LastLogin = DateTime.Now
                };

                SaveProfile(profile);

                var pathHtml = Path.Combine(AppContext.BaseDirectory, "Resources", "auth-success.html");
                var html = File.ReadAllText(pathHtml, Encoding.UTF8)
                    .Replace("{{USERNAME}}", WebUtility.HtmlEncode(username));

                var buffer = Encoding.UTF8.GetBytes(html);

                context.Response.ContentType = "text/html; charset=utf-8";
                context.Response.ContentLength64 = buffer.Length;
                context.Response.OutputStream.Write(buffer, 0, buffer.Length);
                context.Response.OutputStream.Close();

                listener.Stop();

                return new AuthResult { Success = true, Profile = profile };
            }
            catch (Exception ex)
            {
                return new AuthResult { Success = false, ErrorMessage = $"Ошибка OAuth: {ex.Message}" };
            }
        }

    }
}