using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Animation;

namespace LemiCraft_Launcher.Windows
{
    public partial class TwoFactorInputWindow : Window
    {
        public string? Code { get; private set; }

        private TwoFactorInputWindow()
        {
            InitializeComponent();

            ShowInTaskbar = true;
            Topmost = true;

            Opacity = 0;
            RootBorder.RenderTransform = new ScaleTransform(0.95, 0.95);
            Loaded += (s, e) =>
            {
                var fadeIn = new DoubleAnimation(0, 1, TimeSpan.FromMilliseconds(200))
                {
                    EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
                };
                BeginAnimation(OpacityProperty, fadeIn);

                var scale = (ScaleTransform)RootBorder.RenderTransform;
                var scaleAnim = new DoubleAnimation(0.95, 1.0, TimeSpan.FromMilliseconds(200))
                {
                    EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
                };
                scale.BeginAnimation(ScaleTransform.ScaleXProperty, scaleAnim);
                scale.BeginAnimation(ScaleTransform.ScaleYProperty, scaleAnim);

                CodeTextBox.Focus();
            };
        }

        private void TitleBar_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.ChangedButton == MouseButton.Left)
                DragMove();
        }

        private void CodeTextBox_KeyDown(object sender, System.Windows.Input.KeyEventArgs e)
        {
            if (e.Key == Key.Enter)
                ConfirmButton_Click(sender, e);
            else if (e.Key == Key.Escape)
                CancelButton_Click(sender, e);
        }

        private void ConfirmButton_Click(object sender, RoutedEventArgs e)
        {
            var value = CodeTextBox.Text.Trim();
            if (string.IsNullOrWhiteSpace(value))
            {
                ErrorText.Text = "Введите код";
                ErrorText.Visibility = Visibility.Visible;
                return;
            }

            Code = value;
            CloseWithAnimation();
        }

        private void CancelButton_Click(object sender, RoutedEventArgs e)
        {
            Code = null;
            CloseWithAnimation();
        }

        private void CloseWithAnimation()
        {
            var fadeOut = new DoubleAnimation(1, 0, TimeSpan.FromMilliseconds(150))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn }
            };
            fadeOut.Completed += (s, _) => Close();
            BeginAnimation(OpacityProperty, fadeOut);

            if (RootBorder.RenderTransform is ScaleTransform st)
            {
                var scaleAnim = new DoubleAnimation(1, 0.95, TimeSpan.FromMilliseconds(150))
                {
                    EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn }
                };
                st.BeginAnimation(ScaleTransform.ScaleXProperty, scaleAnim);
                st.BeginAnimation(ScaleTransform.ScaleYProperty, scaleAnim);
            }
        }

        /// <summary>
        /// Показывает модальное окно ввода 2FA-кода. Возвращает введённый код
        /// или null, если пользователь отменил ввод.
        /// </summary>
        public static string? RequestCode()
        {
            var window = new TwoFactorInputWindow();
            window.ShowDialog();
            return window.Code;
        }
    }
}