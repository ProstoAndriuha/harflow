import type { TranslationDict } from './en.js';

export const ru: TranslationDict = {
  appName: 'HARFlow Studio',
  offlineBadge: 'Офлайн · Секреты замаскированы',
  themeDark: 'Тёмная',
  themeLight: 'Светлая',
  nav: {
    dashboard: 'Дашборд',
    traffic: 'Трафик',
    dependencies: 'Граф зависимостей',
    settings: 'Настройки',
  },
  dashboard: {
    title: 'Обзор проекта',
    createDemo: 'Загрузить синтетический демо-проект',
    importHarLabel: 'Импорт локального HAR (.har / .json)',
    cards: {
      totalRequests: 'Всего запросов',
      uniqueEndpoints: 'Уникальных endpoint',
      apiDomains: 'API-домены',
      dependencies: 'Зависимости',
      authFlows: 'Потоки авторизации',
      unknownMappings: 'Неизвестные связи',
      exportReadiness: 'Готовность к экспорту',
    },
    readiness: {
      not_ready: 'Не готов',
      partial: 'Частично',
      ready: 'Готов',
    },
  },
  traffic: {
    title: 'Обозреватель трафика',
    empty: 'Записи трафика ещё не загружены.',
    detailsTitle: 'Детали очищенного запроса',
    classificationReason: 'Причина классификации',
    sanitizedHeaders: 'Очищенные заголовки',
    columns: {
      method: 'Метод',
      status: 'Статус',
      endpoint: 'Endpoint',
      domain: 'Домен',
      duration: 'Длительность',
      size: 'Размер',
      type: 'Тип',
      session: 'Сессия',
    },
  },
  dependencies: {
    title: 'Граф зависимостей и доказательства',
    confirm: 'Подтвердить',
    reject: 'Отклонить',
    confidence: 'Уверенность',
    evidence: 'Доказательства',
    empty: 'Зависимости ещё не обнаружены.',
  },
  settings: {
    title: 'Настройки',
    appearance: 'Тема оформления',
    language: 'Язык интерфейса',
    privacyTitle: 'Приватность и офлайн-режим',
    privacyBody:
      'Весь разбор HAR и маскирование секретов выполняются локально. Исходные Authorization, Cookie, Set-Cookie и ключи API удаляются до сохранения.',
    adapterMode: 'Режим адаптера приложения',
  },
};
