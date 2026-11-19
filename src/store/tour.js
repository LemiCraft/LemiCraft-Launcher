import { reactive } from 'vue';

// The guide for a newcomer. A step is a place on the screen (a spotlight), a page to be on and a few words.
// Most steps only show; the steps with a `task` ask the player to do something (log in, choose a skin, play),
// can be put off, and wait for the player where the thing takes a window of its own (the login).
export const TOUR_STEPS = [
  {
    // the greeting: no place is lit, the player is asked whether they want the guide at all
    welcome: true,
    target: null,
    page: 'home',
    placement: 'center',
    title: 'Добро пожаловать в LemiCraft',
    text: 'Новости сервера, скины, моды и запуск игры в одном месте. Хотите, быстро покажу, что где находится, и помогу начать играть?',
  },
  {
    target: '[data-tour="nav-home"]',
    page: 'home',
    placement: 'right',
    title: 'Главная',
    text: 'Новости сервера, кнопка «Играть» и список тех, кто сейчас в игре',
  },
  {
    target: '[data-tour="online"]',
    page: 'home',
    placement: 'bottom',
    title: 'Кто сейчас онлайн',
    text: 'Наведите на плашку, чтобы увидеть всех, и нажмите на игрока, чтобы открыть его профиль',
  },
  {
    target: '[data-tour="nav-skins"]',
    page: 'skins',
    placement: 'right',
    title: 'Скины',
    text: 'Здесь ваши скины: загружайте новые, смотрите их в 3D и надевайте в один клик',
  },
  {
    target: '[data-tour="nav-mods"]',
    page: 'mods',
    placement: 'right',
    title: 'Моды',
    text: 'Каталог модов, наборы по коду и с Modrinth. Всё ставится и удаляется одной кнопкой',
  },
  {
    target: '[data-tour="nav-settings"]',
    page: 'settings',
    placement: 'right',
    title: 'Настройки',
    text: 'Память и профиль производительности, подключение к серверу, уведомления',
  },
  {
    task: 'login',
    // the menu of the account is opened, so that the button of the login is seen where it is
    opensAccountMenu: true,
    target: '[data-tour="login-btn"], [data-tour="account"]',
    page: 'home',
    placement: 'right',
    title: 'Войдите в аккаунт',
    text: 'Для игры на сервере и для скинов нужен аккаунт Microsoft или Ely.by. Вход откроется в отдельном окне',
  },
  {
    task: 'skin',
    target: '[data-tour="upload-skin"]',
    page: 'skins',
    placement: 'bottom',
    title: 'Выберите скин',
    text: 'Загрузите файл со своим скином, потом наденьте его в списке. Если скина пока нет, этот шаг можно пропустить',
  },
  {
    task: 'play',
    target: '[data-tour="play"]',
    page: 'home',
    placement: 'bottom',
    title: 'Установите и запустите игру',
    text: 'Лаунчер сам скачает Java, игру и моды, это несколько минут и пара гигабайт места. Ход установки виден внизу окна. Если места или времени сейчас нет, пропустите шаг и вернитесь позже',
  },
];

const DONE_KEY = 'lc-tour-done';

export const tourState = reactive({
  open: false,
  step: 0,
  // the guide is out of the way while a window of its own (the login) works, and it comes back after
  paused: false,
  // raised by the step of the login; the sidebar answers it by opening its choice of the way to log in
  loginRequests: 0,
  // the same for the dialog of the upload of a skin, and whether it has been opened during this step
  uploadRequests: 0,
  uploadOpened: false,
  // the menu of the account shows itself during the step of the login
  accountMenu: false,
});

function wasShown() {
  try {
    return localStorage.getItem(DONE_KEY) === '1';
  } catch {
    // without storage it would be shown at every start, so it is not shown by itself
    return true;
  }
}

function markShown() {
  try {
    localStorage.setItem(DONE_KEY, '1');
  } catch {
    // nothing to keep it in: the next start asks again
  }
}

export function startTour() {
  tourState.step = 0;
  tourState.uploadOpened = false;
  tourState.paused = false;
  tourState.open = true;
}

export function finishTour() {
  tourState.open = false;
  tourState.accountMenu = false;
  tourState.paused = false;
  markShown();
}

export function nextTourStep() {
  if (tourState.step >= TOUR_STEPS.length - 1) finishTour();
  else tourState.step += 1;
}

export function previousTourStep() {
  if (tourState.step > 0) tourState.step -= 1;
}

/** The first start of the launcher on this machine: the guide begins by itself, once */
export function startTourOnFirstRun() {
  if (!wasShown()) startTour();
}
