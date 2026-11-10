import type { Lang } from '../site-labels';

/** Visible texts of the consent banner and the footer link, per page language. */
export const consentCopy = {
  en: {
    region: 'Cookie consent',
    title: 'Analytics cookies',
    text: 'We would like to use Google Analytics cookies to count visits. Nothing is loaded or stored until you choose. You can change your mind at any time.',
    more: 'Cookie policy',
    accept: 'Accept',
    reject: 'Reject',
    settings: 'Cookie settings',
    policyHref: '/cookies/',
  },
  es: {
    region: 'Consentimiento de cookies',
    title: 'Cookies de analítica',
    text: 'Nos gustaría usar cookies de Google Analytics para contar las visitas. No se carga ni se guarda nada hasta que elijas. Puedes cambiar de opinión cuando quieras.',
    more: 'Política de cookies',
    accept: 'Aceptar',
    reject: 'Rechazar',
    settings: 'Configuración de cookies',
    policyHref: '/es/cookies/',
  },
} as const satisfies Record<Lang, Record<string, string>>;
