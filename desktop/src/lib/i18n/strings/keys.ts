// GENERERT av tools/gen-i18n fra i18n/*.json (just i18n-gen). Ikke rediger for hånd.

/** Parametre per nøkkel (plural -> number, select -> string, ellers string | number). */
export interface MessageParams {
  "onb.start.button": Record<never, never>;
  "home.ring.caption": { size: string | number };
  "review.bar.trash": { count: number; size: string | number };
  "reason.exact_copy": Record<never, never>;
  "reason.visual_duplicate": Record<never, never>;
  "reason.higher_resolution": Record<never, never>;
  "reason.less_compressed": Record<never, never>;
  "reason.sharper": Record<never, never>;
  "reason.older": Record<never, never>;
  "reason.screenshot": Record<never, never>;
  "reason.shot_platform_flag": Record<never, never>;
  "reason.shot_screen_size": Record<never, never>;
  "reason.shot_no_camera_exif": Record<never, never>;
  "reason.shot_png": Record<never, never>;
  "reason.blurry": { pct: string | number };
  "trash.freed.note": Record<never, never>;
  "app.name": Record<never, never>;
  "common.cancel": Record<never, never>;
  "common.continue": Record<never, never>;
  "common.close": Record<never, never>;
  "common.undo": Record<never, never>;
  "common.retry": Record<never, never>;
  "common.back": Record<never, never>;
  "common.done": Record<never, never>;
  "common.loading": Record<never, never>;
  "common.zoom": Record<never, never>;
  "common.shortcuts": Record<never, never>;
  "common.dismiss": Record<never, never>;
  "common.count.photos": { count: number };
  "onb.welcome.title": Record<never, never>;
  "onb.welcome.body": Record<never, never>;
  "onb.folders.title": Record<never, never>;
  "onb.folders.privacy": Record<never, never>;
  "onb.folders.default": Record<never, never>;
  "onb.folders.add": Record<never, never>;
  "onb.folders.remove": Record<never, never>;
  "onb.folders.start": Record<never, never>;
  "onb.noaccess.title": Record<never, never>;
  "onb.noaccess.body": Record<never, never>;
  "onb.noaccess.choose": Record<never, never>;
  "onb.scan.title": Record<never, never>;
  "onb.scan.found": { count: number };
  "onb.scan.metadata": Record<never, never>;
  "onb.partial.title": Record<never, never>;
  "onb.partial.body": Record<never, never>;
  "onb.result.summary": { count: number; size: string | number };
  "onb.result.primary": { count: number };
  "onb.result.review": Record<never, never>;
  "onb.nothing.body": Record<never, never>;
  "onb.done.title": { count: number };
  "onb.done.body": { size: string | number };
  "onb.done.home": Record<never, never>;
  "home.title": Record<never, never>;
  "home.ring.label": { size: string | number; trash: string | number };
  "home.ring.trash": { size: string | number };
  "home.cat.exact_dup": Record<never, never>;
  "home.cat.visual_dup": Record<never, never>;
  "home.cat.blurry": Record<never, never>;
  "home.cat.screenshot": Record<never, never>;
  "home.cat.summary": { count: number; size: string | number };
  "home.quick.button": Record<never, never>;
  "home.scan.progress": { done: string | number; total: string | number };
  "home.scan.pause": Record<never, never>;
  "home.scan.cancel": Record<never, never>;
  "home.trash.link": { count: string | number };
  "home.expired.banner": { count: number };
  "home.partial.banner": Record<never, never>;
  "home.partial.show": Record<never, never>;
  "home.clean.body": Record<never, never>;
  "home.error.body": Record<never, never>;
  "review.title": Record<never, never>;
  "review.back": Record<never, never>;
  "review.select.all": Record<never, never>;
  "review.select.none": Record<never, never>;
  "review.select.photo": { name: string | number };
  "review.select.state.on": Record<never, never>;
  "review.select.state.off": Record<never, never>;
  "review.open": Record<never, never>;
  "review.group.row": { count: number };
  "review.group.best": Record<never, never>;
  "review.protected.title": Record<never, never>;
  "review.protected.note": Record<never, never>;
  "review.bar.none": Record<never, never>;
  "review.bar.selected": { count: number };
  "review.committing": Record<never, never>;
  "review.committed": { count: number };
  "review.empty.title": Record<never, never>;
  "review.empty.body": Record<never, never>;
  "review.error.body": Record<never, never>;
  "review.more": Record<never, never>;
  "review.confirm.title": Record<never, never>;
  "review.confirm.body": { days: string | number };
  "review.confirm.ack": Record<never, never>;
  "review.confirm.go": Record<never, never>;
  "review.photo.name": { n: string | number; size: string | number; total: string | number };
  "review.confirm.preview": { count: number; size: string | number };
  "group.title": Record<never, never>;
  "group.exact.note": Record<never, never>;
  "group.keep.primary": { count: number };
  "group.keep.all": Record<never, never>;
  "group.make.best": Record<never, never>;
  "group.best": Record<never, never>;
  "group.prev": Record<never, never>;
  "group.next": Record<never, never>;
  "group.zoom.in": Record<never, never>;
  "group.zoom.out": Record<never, never>;
  "group.info.resolution": Record<never, never>;
  "group.info.size": Record<never, never>;
  "group.info.date": Record<never, never>;
  "group.info.sharpness": Record<never, never>;
  "group.info.sharper": Record<never, never>;
  "group.committed": { count: number };
  "group.back": Record<never, never>;
  "group.error.body": Record<never, never>;
  "group.keys.hint": Record<never, never>;
  "group.protected": Record<never, never>;
  "group.item.name": { n: string | number; total: string | number };
  "cards.title": Record<never, never>;
  "cards.keep": Record<never, never>;
  "cards.trash": Record<never, never>;
  "cards.skip": Record<never, never>;
  "cards.overlay.keep": Record<never, never>;
  "cards.overlay.trash": Record<never, never>;
  "cards.overlay.skip": Record<never, never>;
  "cards.remaining": { count: number };
  "cards.loading": Record<never, never>;
  "cards.empty.title": Record<never, never>;
  "cards.summary": { kept: string | number; trashed: number };
  "cards.undo.last": Record<never, never>;
  "cards.error.body": Record<never, never>;
  "cards.help.title": Record<never, never>;
  "cards.photo.alt": { n: string | number; total: string | number };
  "cards.drag.hint": Record<never, never>;
  "cards.key.space": Record<never, never>;
  "trash.title": Record<never, never>;
  "trash.summary": { count: number; size: string | number };
  "trash.section.expired": Record<never, never>;
  "trash.section.soon": Record<never, never>;
  "trash.section.other": Record<never, never>;
  "trash.row.moved": { date: string | number };
  "trash.row.expires": { days: number };
  "trash.row.expired": Record<never, never>;
  "trash.restore": Record<never, never>;
  "trash.restore.all": Record<never, never>;
  "trash.empty.button": Record<never, never>;
  "trash.empty.title": Record<never, never>;
  "trash.purge.title": Record<never, never>;
  "trash.purge.summary": { count: number; size: string | number };
  "trash.purge.mode.recycle": Record<never, never>;
  "trash.purge.mode.permanent": Record<never, never>;
  "trash.purge.nobin": Record<never, never>;
  "trash.permanent.title": Record<never, never>;
  "trash.permanent.ack": Record<never, never>;
  "trash.permanent.go": { count: number };
  "trash.purged.recycle": Record<never, never>;
  "trash.purged.permanent": { size: string | number };
  "trash.purge.marked": { count: number };
  "trash.history": { date: string | number; size: string | number };
  "trash.error.body": Record<never, never>;
  "consequence.protected_favorite": Record<never, never>;
  "consequence.protected_shared_album": Record<never, never>;
  "consequence.protected_edited": Record<never, never>;
  "consequence.in_album": Record<never, never>;
  "consequence.not_backed_up": Record<never, never>;
  "consequence.backup_unknown": Record<never, never>;
  "consequence.mass_delete": Record<never, never>;
  "consequence.frees_no_storage_yet": Record<never, never>;
  "consequence.platform_dialog_next": Record<never, never>;
  "consequence.expired_notice": Record<never, never>;
  "consequence.ack_required": Record<never, never>;
  "consequence.plan_stale": Record<never, never>;
  "consequence.purge_stale": Record<never, never>;
  "consequence.not_undoable": Record<never, never>;
  "consequence.already_purged": Record<never, never>;
  "consequence.unknown_asset": Record<never, never>;
  "err.generic": Record<never, never>;
  "err.nothing_to_undo": Record<never, never>;
  "err.unsupported": Record<never, never>;
  "folders.title": Record<never, never>;
  "folders.unreadable": Record<never, never>;
  "folders.offline": Record<never, never>;
  "folders.list.label": Record<never, never>;
  "settings.title": Record<never, never>;
  "setting.language": Record<never, never>;
  "setting.language.system": Record<never, never>;
  "setting.language.nb": Record<never, never>;
  "setting.language.en": Record<never, never>;
  "setting.trash.days": Record<never, never>;
  "setting.trash.days.value": { days: number };
  "setting.sound": Record<never, never>;
  "setting.sound.note": Record<never, never>;
  "setting.notify.expired": Record<never, never>;
  "restore.done": { count: number };
  "restore.failed": Record<never, never>;
  "restore.where": Record<never, never>;
  "restore.list.label": Record<never, never>;
  "restore.row.select": { name: string | number };
  "restore.selected": { count: number };
  "restore.none": Record<never, never>;
  "home.folder.add": Record<never, never>;
  "home.folder.title": Record<never, never>;
  "home.folder.path": Record<never, never>;
  "home.folder.example": Record<never, never>;
  "home.folder.note": Record<never, never>;
  "home.folder.submit": Record<never, never>;
  "home.folder.error": Record<never, never>;
  "home.folder.current": { path: string | number };
  "home.folder.none": Record<never, never>;
  "home.idle.body": Record<never, never>;
  "home.scan.start": Record<never, never>;
  "home.scan.paused": Record<never, never>;
  "home.scan.resume": Record<never, never>;
  "home.scan.cancelled": Record<never, never>;
  "home.scan.error": Record<never, never>;
  "home.scan.controls": Record<never, never>;
  "settings.language.note": Record<never, never>;
  "settings.trash.hint": Record<never, never>;
  "settings.quarantine.title": Record<never, never>;
  "settings.quarantine.body": Record<never, never>;
  "settings.quarantine.path": { path: string | number };
  "settings.quarantine.none": Record<never, never>;
  "settings.empty.title": Record<never, never>;
  "settings.empty.default": Record<never, never>;
  "settings.saved": Record<never, never>;
  "settings.error": Record<never, never>;
  "help.for": { screen: string | number };
  "help.screen.onboarding": Record<never, never>;
  "help.screen.group": Record<never, never>;
  "help.screen.cards": Record<never, never>;
  "help.section.everywhere": Record<never, never>;
  "help.section.screen": Record<never, never>;
  "help.action.help": Record<never, never>;
  "help.action.close": Record<never, never>;
  "help.action.toggle": Record<never, never>;
  "help.action.trash_selected": Record<never, never>;
  "help.action.best": Record<never, never>;
  "help.action.tab": Record<never, never>;
  "help.action.activate": Record<never, never>;
  "a11y.photo.label": { date: string | number; n: string | number; size: string | number; total: string | number };
  "a11y.photo.best": Record<never, never>;
  "a11y.photo.reason": { reason: string | number };
  "a11y.live.trashed": { left: string | number };
  "a11y.live.kept": { left: string | number };
  "a11y.live.skipped": { left: string | number };
  "a11y.live.undone": Record<never, never>;
  "a11y.live.selected": { count: number; size: string | number };
  "a11y.live.scan": { done: string | number; total: string | number };
  "a11y.live.restored": { count: number };
  "a11y.keys.up": Record<never, never>;
  "a11y.keys.left": Record<never, never>;
  "a11y.keys.right": Record<never, never>;
  "a11y.keys.undo": Record<never, never>;
  "a11y.keys.focusmode": Record<never, never>;
  "a11y.ring": Record<never, never>;
  "a11y.skip.main": Record<never, never>;
  "a11y.dialog.close": Record<never, never>;
  "ui.card.suggestion": Record<never, never>;
  "ui.badge.protected": Record<never, never>;
  "ui.bar.label": Record<never, never>;
  "ui.sheet.label": Record<never, never>;
  "ui.progress.label": Record<never, never>;
  "ui.livebox.label": Record<never, never>;
  "ui.empty.title": Record<never, never>;
  "ui.error.title": Record<never, never>;
  "dev.state.backend": Record<never, never>;
  "dev.state.scan": Record<never, never>;
  "dev.state.reclaimable": Record<never, never>;
  "dev.state.trash": Record<never, never>;
  "nav.label": Record<never, never>;
  "nav.home": Record<never, never>;
  "nav.review": Record<never, never>;
  "nav.trash": Record<never, never>;
  "nav.settings": Record<never, never>;
  "update.title": Record<never, never>;
  "update.version": { version: string | number };
  "update.check": Record<never, never>;
  "update.install": Record<never, never>;
  "update.status.checking": Record<never, never>;
  "update.status.none": Record<never, never>;
  "update.status.available": { version: string | number };
  "update.released": { date: string | number };
  "update.notes.title": Record<never, never>;
  "update.error.check": Record<never, never>;
  "update.error.unsupported": Record<never, never>;
  "update.status.installing": { version: string | number };
  "update.progress.label": Record<never, never>;
  "update.progress.known": { done: string | number; percent: string | number; total: string | number };
  "update.progress.unknown": { done: string | number };
  "update.status.restarting": Record<never, never>;
  "update.error.install": Record<never, never>;
  "update.blocked": Record<never, never>;
  "update.auto.label": Record<never, never>;
  "update.auto.note": Record<never, never>;
  "update.auto.error": Record<never, never>;
  "home.update.banner": { version: string | number };
}

export type MessageKey = keyof MessageParams;

/** Nøkler for knapper, faner og merker (`len: short`): kort plass i layout. */
export const SHORT_KEYS: ReadonlySet<MessageKey> = new Set<MessageKey>([
  "onb.start.button",
  "review.bar.trash",
  "reason.exact_copy",
  "reason.visual_duplicate",
  "reason.higher_resolution",
  "reason.less_compressed",
  "reason.sharper",
  "reason.older",
  "reason.screenshot",
  "reason.shot_platform_flag",
  "reason.shot_screen_size",
  "reason.shot_no_camera_exif",
  "reason.shot_png",
  "reason.blurry",
  "common.cancel",
  "common.continue",
  "common.close",
  "common.undo",
  "common.retry",
  "common.back",
  "common.done",
  "common.zoom",
  "common.shortcuts",
  "common.dismiss",
  "onb.folders.default",
  "onb.folders.add",
  "onb.folders.remove",
  "onb.folders.start",
  "onb.noaccess.choose",
  "onb.result.primary",
  "onb.result.review",
  "onb.done.home",
  "home.title",
  "home.cat.exact_dup",
  "home.cat.visual_dup",
  "home.cat.blurry",
  "home.cat.screenshot",
  "home.quick.button",
  "home.scan.pause",
  "home.scan.cancel",
  "home.trash.link",
  "home.partial.show",
  "review.title",
  "review.back",
  "review.select.all",
  "review.select.none",
  "review.select.state.on",
  "review.select.state.off",
  "review.open",
  "review.group.best",
  "review.bar.none",
  "review.bar.selected",
  "review.committing",
  "review.committed",
  "review.confirm.go",
  "group.title",
  "group.keep.primary",
  "group.keep.all",
  "group.make.best",
  "group.best",
  "group.prev",
  "group.next",
  "group.zoom.in",
  "group.zoom.out",
  "group.back",
  "cards.title",
  "cards.keep",
  "cards.trash",
  "cards.skip",
  "cards.overlay.keep",
  "cards.overlay.trash",
  "cards.overlay.skip",
  "cards.remaining",
  "cards.undo.last",
  "cards.key.space",
  "trash.title",
  "trash.row.expired",
  "trash.restore",
  "trash.restore.all",
  "trash.empty.button",
  "trash.permanent.go",
  "folders.title",
  "settings.title",
  "setting.language",
  "setting.language.system",
  "setting.sound",
  "restore.selected",
  "home.folder.add",
  "home.folder.submit",
  "home.scan.start",
  "home.scan.resume",
  "home.scan.controls",
  "settings.quarantine.title",
  "settings.empty.title",
  "settings.saved",
  "help.screen.onboarding",
  "help.screen.group",
  "help.screen.cards",
  "help.section.everywhere",
  "a11y.skip.main",
  "a11y.dialog.close",
  "ui.card.suggestion",
  "ui.badge.protected",
  "nav.home",
  "nav.review",
  "nav.trash",
  "nav.settings",
  "update.title",
  "update.check",
  "update.install",
  "update.auto.label",
]);
