import type * as Monaco from "@codingame/monaco-vscode-editor-api";

const HOVER_DELAY_MS = 260;
const HOVER_HIDE_DELAY_MS = 320;
const HOVER_START_MODE_IMMEDIATE = 1;
const HOVER_START_SOURCE_MOUSE = 0;
const INTERACTIVE_WIDGET_SELECTOR = ".monaco-hover, .workbench-hover-container, .context-view, .action-widget";

interface HoverBridgeOptions {
  monaco: typeof Monaco;
  editor: Monaco.editor.IStandaloneCodeEditor;
  root: ShadowRoot;
  ownsModel(model: Monaco.editor.ITextModel): boolean;
}

interface ContentHoverController {
  shouldKeepOpenOnEditorMouseMoveOrLeave?: boolean;
  _contentWidget?: ContentHoverWidgetWrapper;
  showContentHover(range: Monaco.Range, mode: number, source: number, focus: boolean): void;
}

interface ContentHoverWidgetWrapper {
  getDomNode(): HTMLElement;
  hide(): void;
  isVisible?: boolean;
}

interface Point {
  x: number;
  y: number;
}

export function installShadowDomHoverBridge(options: HoverBridgeOptions): Monaco.IDisposable {
  let showTimer: number | undefined;
  let hideTimer: number | undefined;
  let lastPoint: Point | undefined;
  const ownerDocument = options.editor.getDomNode()?.ownerDocument ?? document;

  const controller = (): ContentHoverController | null =>
    options.editor.getContribution("editor.contrib.contentHover") as ContentHoverController | null;

  const hoverWidget = (): ContentHoverWidgetWrapper | null => controller()?._contentWidget ?? null;

  const setKeepOpen = (keepOpen: boolean) => {
    const contentHoverController = controller();
    if (contentHoverController) {
      contentHoverController.shouldKeepOpenOnEditorMouseMoveOrLeave = keepOpen;
    }
  };

  const clearShowTimer = () => {
    if (showTimer !== undefined) {
      window.clearTimeout(showTimer);
      showTimer = undefined;
    }
  };

  const clearHideTimer = () => {
    if (hideTimer !== undefined) {
      window.clearTimeout(hideTimer);
      hideTimer = undefined;
    }
  };

  const pointInsideRect = (point: Point | undefined, element: HTMLElement | null): boolean => {
    if (!point || !element) {
      return false;
    }
    const rect = element.getBoundingClientRect();
    return point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom;
  };

  const pointInsideEditor = (point: Point | undefined): boolean => {
    return pointInsideRect(point, options.editor.getDomNode());
  };

  const pointInsideHover = (point: Point | undefined): boolean => {
    return pointInsideRect(point, hoverWidget()?.getDomNode() ?? null) || pointInsideInteractiveWidget(point);
  };

  const pointInsideInteractiveArea = (point: Point | undefined): boolean => pointInsideEditor(point) || pointInsideHover(point);

  const hideHover = () => {
    hoverWidget()?.hide();
    setKeepOpen(false);
  };

  const scheduleHideIfOutsideInteractiveArea = (point: Point | undefined) => {
    if (pointInsideInteractiveArea(point)) {
      clearHideTimer();
      return;
    }

    clearHideTimer();
    setKeepOpen(true);
    hideTimer = window.setTimeout(() => {
      hideTimer = undefined;
      if (!pointInsideInteractiveArea(point)) {
        hideHover();
      }
    }, HOVER_HIDE_DELAY_MS);
  };

  const syncKeepOpen = (point: Point) => {
    if (pointInsideHover(point)) {
      clearHideTimer();
      clearShowTimer();
      setKeepOpen(true);
      return;
    }

    if (pointInsideEditor(point)) {
      clearHideTimer();
      setKeepOpen(false);
      return;
    }

    if (hoverWidget()?.isVisible) {
      scheduleHideIfOutsideInteractiveArea(point);
    }
  };

  const scheduleHoverAtPoint = (point: Point) => {
    if (!pointInsideEditor(point)) {
      clearShowTimer();
      return;
    }

    const target = options.editor.getTargetAtClientPoint(point.x, point.y);
    if (target?.type !== options.monaco.editor.MouseTargetType.CONTENT_TEXT || !target.range) {
      if (!hoverWidget()?.isVisible) {
        clearShowTimer();
      }
      return;
    }

    const model = options.editor.getModel();
    if (!model || !options.ownsModel(model)) {
      clearShowTimer();
      return;
    }

    clearShowTimer();
    clearHideTimer();
    setKeepOpen(false);
    const range = target.range;
    showTimer = window.setTimeout(() => {
      showTimer = undefined;
      if (lastPoint && pointInsideEditor(lastPoint)) {
        controller()?.showContentHover(range, HOVER_START_MODE_IMMEDIATE, HOVER_START_SOURCE_MOUSE, false);
        setKeepOpen(true);
      }
    }, HOVER_DELAY_MS);
  };

  const pointInsideInteractiveWidget = (point: Point | undefined): boolean => {
    if (!point) {
      return false;
    }
    return [options.root.elementFromPoint(point.x, point.y), ownerDocument.elementFromPoint(point.x, point.y)].some((element) =>
      element?.closest(INTERACTIVE_WIDGET_SELECTOR),
    );
  };

  const documentMouseMove = (event: MouseEvent) => {
    lastPoint = { x: event.clientX, y: event.clientY };
    syncKeepOpen(lastPoint);
    scheduleHoverAtPoint(lastPoint);
  };
  ownerDocument.addEventListener("mousemove", documentMouseMove, true);

  return {
    dispose() {
      clearShowTimer();
      clearHideTimer();
      ownerDocument.removeEventListener("mousemove", documentMouseMove, true);
      setKeepOpen(false);
    },
  };
}
