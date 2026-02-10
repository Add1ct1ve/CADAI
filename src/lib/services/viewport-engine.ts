import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TransformControls } from 'three/addons/controls/TransformControls.js';
import { STLLoader } from 'three/addons/loaders/STLLoader.js';
import { ViewHelper } from 'three/addons/helpers/ViewHelper.js';
import type { ObjectId, PrimitiveParams, PrimitiveType, CadTransform, CameraState, Sketch, SketchId, SketchEntityId, SketchToolId, ConstraintState, Point2D, DatumId, DisplayMode, SectionPlaneConfig, MeasurementId } from '$lib/types/cad';
import { getDefaultParams } from '$lib/types/cad';
import { cadToThreePos, cadToThreeRot } from '$lib/services/coord-utils';
import { SketchRenderer } from '$lib/services/sketch-renderer';
import { getSketchPlaneInfo, threeToSketchPos, getSketchViewCamera, snapToSketchGrid } from '$lib/services/sketch-plane-utils';

const DEFAULT_COLOR = 0x89b4fa;
const SELECTED_EMISSIVE = 0x335588;
const HOVERED_EMISSIVE = 0x1a2a44;
const SELECTED_OUTLINE_COLOR = 0x89b4fa;
const HOVERED_OUTLINE_COLOR = 0x4a6a8a;

type TransformMode = 'translate' | 'rotate' | 'scale';
type TransformCallback = (id: ObjectId, group: THREE.Group) => void;
type ScaleEndCallback = (id: ObjectId, scale: THREE.Vector3) => void;

export class ViewportEngine {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private container: HTMLElement;
  private animationId: number | null = null;
  private resizeObserver: ResizeObserver;
  private grid: THREE.GridHelper;
  private axes: THREE.AxesHelper;
  private viewHelper: ViewHelper;
  private clock: THREE.Clock;
  private _isAnimatingView = false;

  // Legacy single model (for manual mode / STL loading)
  private currentModel: THREE.Mesh | THREE.Group | null = null;

  // CAD tool object meshes
  private objectMeshes: Map<ObjectId, THREE.Group> = new Map();
  private selectedIds: Set<ObjectId> = new Set();
  private hoveredIdInternal: ObjectId | null = null;

  // Placement preview ghost
  private ghostMesh: THREE.Group | null = null;

  // Datum geometry meshes
  private datumMeshes: Map<DatumId, THREE.Group> = new Map();

  // Display mode
  private currentDisplayMode: DisplayMode = 'shaded';
  private clippingPlane: THREE.Plane | null = null;
  private sectionPlaneHelper: THREE.PlaneHelper | null = null;

  // Grid config for snapping and rebuild
  private gridCellSize = 1;
  private gridSize = 100;

  // Hemisphere light reference for theme changes
  private hemisphereLight: THREE.HemisphereLight;

  // Measurement overlays
  private measurementGroup: THREE.Group;
  private measurementMeshes: Map<MeasurementId, THREE.Group> = new Map();
  private pendingMarker: THREE.Mesh | null = null;

  // Sketch support
  private sketchRenderer: SketchRenderer;

  // Raycaster
  private raycaster = new THREE.Raycaster();
  private ndcMouse = new THREE.Vector2();

  // TransformControls
  private transformControls: TransformControls;
  private transformMode: TransformMode | null = null;
  private attachedObjectId: ObjectId | null = null;
  private _isTransformDragging = false;
  private transformChangeCb: TransformCallback | null = null;
  private transformEndCb: TransformCallback | null = null;
  private scaleEndCb: ScaleEndCallback | null = null;

  constructor(container: HTMLElement) {
    this.container = container;

    // Scene
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x1a1a2e);

    // Camera
    const { clientWidth: w, clientHeight: h } = container;
    this.camera = new THREE.PerspectiveCamera(50, w / h || 1, 0.1, 1000);
    this.camera.position.set(8, 6, 8);
    this.camera.lookAt(0, 0, 0);

    // Renderer
    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: true,
    });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setSize(w, h);
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.0;
    container.appendChild(this.renderer.domElement);

    // OrbitControls
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 2;
    this.controls.maxDistance = 100;
    this.controls.target.set(0, 0, 0);

    // TransformControls
    this.transformControls = new TransformControls(this.camera, this.renderer.domElement);
    this.transformControls.setSize(0.75);
    this.scene.add(this.transformControls.getHelper());

    // Disable orbit while dragging gizmo
    this.transformControls.addEventListener('dragging-changed', (event) => {
      const dragging = (event as unknown as { value: boolean }).value;
      this.controls.enabled = !dragging;
      this._isTransformDragging = dragging;
    });

    // Live transform change callback
    this.transformControls.addEventListener('change', () => {
      if (!this._isTransformDragging || !this.attachedObjectId) return;

      const group = this.objectMeshes.get(this.attachedObjectId);
      if (!group) return;

      if (this.transformChangeCb) {
        this.transformChangeCb(this.attachedObjectId, group);
      }
    });

    // Mouse-up: transform end
    this.transformControls.addEventListener('mouseUp', () => {
      if (!this.attachedObjectId) return;

      const group = this.objectMeshes.get(this.attachedObjectId);
      if (!group) return;

      if (this.transformMode === 'scale' && this.scaleEndCb) {
        // Read scale, reset to 1, fire scale callback
        const scale = group.scale.clone();
        group.scale.set(1, 1, 1);
        this.scaleEndCb(this.attachedObjectId, scale);
      } else if (this.transformEndCb) {
        this.transformEndCb(this.attachedObjectId, group);
      }
    });

    // Grid
    this.grid = new THREE.GridHelper(100, 100, 0x404060, 0x2a2a40);
    this.scene.add(this.grid);

    // Axes
    this.axes = new THREE.AxesHelper(5);
    this.scene.add(this.axes);

    // ViewHelper (interactive axis gizmo in bottom-right corner)
    this.viewHelper = new ViewHelper(this.camera, this.renderer.domElement);
    this.viewHelper.center = this.controls.target;

    // Clock for ViewHelper animation
    this.clock = new THREE.Clock();

    // Lighting
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.4);
    this.scene.add(ambientLight);

    const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
    directionalLight.position.set(5, 10, 7);
    directionalLight.castShadow = true;
    directionalLight.shadow.mapSize.width = 1024;
    directionalLight.shadow.mapSize.height = 1024;
    this.scene.add(directionalLight);

    this.hemisphereLight = new THREE.HemisphereLight(0x89b4fa, 0x1a1a2e, 0.3);
    this.scene.add(this.hemisphereLight);

    // Measurement overlay group
    this.measurementGroup = new THREE.Group();
    this.measurementGroup.renderOrder = 10;
    this.scene.add(this.measurementGroup);

    // Sketch renderer
    this.sketchRenderer = new SketchRenderer(this.scene);

    // Resize observer
    this.resizeObserver = new ResizeObserver(() => {
      this.resize();
    });
    this.resizeObserver.observe(container);

    // Start render loop
    this.animate();
  }

  private animate = (): void => {
    this.animationId = requestAnimationFrame(this.animate);
    const delta = this.clock.getDelta();

    if (this.viewHelper.animating) {
      this.viewHelper.update(delta);
    }

    this.controls.update();
    this.renderer.render(this.scene, this.camera);
    this.renderer.autoClear = false;
    this.viewHelper.render(this.renderer);
    this.renderer.autoClear = true;
  };

  // ─── Public API: ViewHelper ─────────────────────────

  handleViewHelperClick(event: PointerEvent): boolean {
    return this.viewHelper.handleClick(event);
  }

  // ─── Public API: Camera ──────────────────────────

  getCamera(): THREE.PerspectiveCamera {
    return this.camera;
  }

  getContainer(): HTMLElement {
    return this.container;
  }

  getCameraState(): CameraState {
    return {
      position: [this.camera.position.x, this.camera.position.y, this.camera.position.z],
      target: [this.controls.target.x, this.controls.target.y, this.controls.target.z],
      zoom: this.camera.zoom,
    };
  }

  setCameraState(state: CameraState): void {
    this.camera.position.set(...state.position);
    this.controls.target.set(...state.target);
    this.camera.zoom = state.zoom;
    this.camera.updateProjectionMatrix();
    this.controls.update();
  }

  // ─── Public API: TransformControls ────────────────

  /**
   * Set the transform gizmo mode, or null to hide it.
   */
  setTransformMode(mode: TransformMode | null): void {
    this.transformMode = mode;
    if (mode) {
      this.transformControls.setMode(mode);
      this.transformControls.enabled = true;
      this.transformControls.getHelper().visible = true;
    } else {
      this.transformControls.enabled = false;
      this.transformControls.getHelper().visible = false;
      this.detachTransform();
    }
  }

  /**
   * Attach the gizmo to a specific object mesh, or null to detach.
   */
  attachTransformToObject(id: ObjectId | null): void {
    if (!id) {
      this.detachTransform();
      return;
    }

    const group = this.objectMeshes.get(id);
    if (!group) {
      this.detachTransform();
      return;
    }

    this.attachedObjectId = id;
    this.transformControls.attach(group);
  }

  private detachTransform(): void {
    this.attachedObjectId = null;
    this.transformControls.detach();
  }

  /**
   * Register a callback fired live during gizmo drag.
   */
  onTransformChange(cb: TransformCallback): void {
    this.transformChangeCb = cb;
  }

  /**
   * Register a callback fired when gizmo drag ends (mouse-up).
   */
  onTransformEnd(cb: TransformCallback): void {
    this.transformEndCb = cb;
  }

  /**
   * Register a callback fired when scale drag ends.
   * Receives the accumulated scale factors; the group's scale is reset to 1.
   */
  onScaleEnd(cb: ScaleEndCallback): void {
    this.scaleEndCb = cb;
  }

  /**
   * Set translation snap increment (or null to disable).
   */
  setTranslationSnap(value: number | null): void {
    this.transformControls.setTranslationSnap(value);
  }

  /**
   * Set rotation snap in degrees (or null to disable).
   */
  setRotationSnap(degrees: number | null): void {
    this.transformControls.setRotationSnap(degrees ? degrees * Math.PI / 180 : null);
  }

  /**
   * Check if the gizmo is currently being dragged.
   */
  isTransformDragging(): boolean {
    return this._isTransformDragging;
  }

  /**
   * Get the Three.js group for an object (for reading position/rotation externally).
   */
  getObjectGroup(id: ObjectId): THREE.Group | undefined {
    return this.objectMeshes.get(id);
  }

  // ─── Public API: Preview Mesh Management ─────────

  /**
   * Add or update a preview mesh from parametric object data.
   * Creates Three.js geometry directly (no Python round-trip).
   */
  addPreviewMesh(
    id: ObjectId,
    params: PrimitiveParams,
    transform: CadTransform,
    color: string,
    metalness?: number,
    roughness?: number,
    opacity?: number,
  ): void {
    // Remove existing if updating
    this.removeObject(id);

    const resolvedMetalness = metalness ?? 0.3;
    const resolvedRoughness = roughness ?? 0.7;
    const resolvedOpacity = opacity ?? 1.0;

    const geometry = this.createGeometry(params);
    const material = new THREE.MeshStandardMaterial({
      color: new THREE.Color(color),
      metalness: resolvedMetalness,
      roughness: resolvedRoughness,
      transparent: resolvedOpacity < 1,
      opacity: resolvedOpacity,
      depthWrite: resolvedOpacity >= 1,
    });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.castShadow = true;
    mesh.receiveShadow = true;
    mesh.userData.baseOpacity = resolvedOpacity;

    // Wrap in group for transform
    const group = new THREE.Group();
    group.add(mesh);
    group.userData.objectId = id;

    // Apply transform (CadQuery Z-up → Three.js Y-up)
    const pos = cadToThreePos(transform.position);
    group.position.copy(pos);

    const rot = cadToThreeRot(transform.rotation);
    group.rotation.copy(rot);

    this.objectMeshes.set(id, group);
    this.scene.add(group);

    // Apply current display mode before selection visuals
    if (this.currentDisplayMode !== 'shaded') {
      this.applyDisplayModeToMesh(mesh);
    }

    // Apply selection/hover visuals if needed
    this.updateMeshVisuals(id);
  }

  /**
   * Remove an object mesh from the scene.
   */
  removeObject(id: ObjectId): void {
    const group = this.objectMeshes.get(id);
    if (!group) return;

    // Detach gizmo if attached to this object
    if (this.attachedObjectId === id) {
      this.detachTransform();
    }

    this.scene.remove(group);
    group.traverse((child) => {
      if (child instanceof THREE.Mesh || child instanceof THREE.LineSegments) {
        child.geometry.dispose();
        if (Array.isArray(child.material)) {
          child.material.forEach((m) => m.dispose());
        } else {
          child.material.dispose();
        }
      }
    });
    this.objectMeshes.delete(id);
  }

  /**
   * Remove all object meshes (e.g. when clearing scene).
   */
  removeAllObjects(): void {
    for (const id of this.objectMeshes.keys()) {
      this.removeObject(id);
    }
  }

  /**
   * Update the transform of an existing object mesh.
   */
  updateObjectTransform(id: ObjectId, transform: CadTransform): void {
    const group = this.objectMeshes.get(id);
    if (!group) return;

    const pos = cadToThreePos(transform.position);
    group.position.copy(pos);

    const rot = cadToThreeRot(transform.rotation);
    group.rotation.copy(rot);
  }

  /**
   * Update material properties of an existing object mesh (no geometry rebuild).
   */
  updateObjectMaterial(id: ObjectId, color: string, metalness?: number, roughness?: number, opacity?: number): void {
    const group = this.objectMeshes.get(id);
    if (!group) return;

    const resolvedMetalness = metalness ?? 0.3;
    const resolvedRoughness = roughness ?? 0.7;
    const resolvedOpacity = opacity ?? 1.0;

    group.traverse((child) => {
      if (child instanceof THREE.Mesh && child.material instanceof THREE.MeshStandardMaterial) {
        child.material.color.set(color);
        child.material.metalness = resolvedMetalness;
        child.material.roughness = resolvedRoughness;
        child.userData.baseOpacity = resolvedOpacity;

        // Only apply per-object opacity in shaded/shaded-edges/section modes
        if (this.currentDisplayMode !== 'transparent' && this.currentDisplayMode !== 'wireframe') {
          child.material.transparent = resolvedOpacity < 1;
          child.material.opacity = resolvedOpacity;
          child.material.depthWrite = resolvedOpacity >= 1;
        }

        child.material.needsUpdate = true;
      }
    });
  }

  // ─── Public API: Selection / Hover ───────────────

  /**
   * Set which objects are selected (visual feedback).
   */
  setSelection(ids: ObjectId[]): void {
    const oldIds = new Set(this.selectedIds);
    this.selectedIds = new Set(ids);

    // Update visuals for changed objects
    for (const id of oldIds) {
      if (!this.selectedIds.has(id)) this.updateMeshVisuals(id);
    }
    for (const id of this.selectedIds) {
      if (!oldIds.has(id)) this.updateMeshVisuals(id);
    }
  }

  /**
   * Set which object is hovered (visual feedback).
   */
  setHover(id: ObjectId | null): void {
    const prevId = this.hoveredIdInternal;
    if (prevId === id) return;

    this.hoveredIdInternal = id;

    if (prevId) this.updateMeshVisuals(prevId);
    if (id) this.updateMeshVisuals(id);
  }

  private updateMeshVisuals(id: ObjectId): void {
    const group = this.objectMeshes.get(id);
    if (!group) return;

    const isSelected = this.selectedIds.has(id);
    const isHovered = this.hoveredIdInternal === id;

    // Remove existing outlines
    const toRemove: THREE.Object3D[] = [];
    group.traverse((child) => {
      if (child.userData.isOutline) toRemove.push(child);
    });
    for (const obj of toRemove) {
      obj.parent?.remove(obj);
      if (obj instanceof THREE.LineSegments) {
        obj.geometry.dispose();
        (obj.material as THREE.Material).dispose();
      }
    }

    group.traverse((child) => {
      if (child instanceof THREE.Mesh && child.material instanceof THREE.MeshStandardMaterial) {
        if (isSelected) {
          child.material.emissive.setHex(SELECTED_EMISSIVE);
          child.material.emissiveIntensity = 0.6;
        } else if (isHovered) {
          child.material.emissive.setHex(HOVERED_EMISSIVE);
          child.material.emissiveIntensity = 0.4;
        } else {
          child.material.emissive.setHex(0x000000);
          child.material.emissiveIntensity = 0;
        }

        // Add edge outline for selected or hovered
        if (isSelected || isHovered) {
          const edgesGeo = new THREE.EdgesGeometry(child.geometry, 30);
          const color = isSelected ? SELECTED_OUTLINE_COLOR : HOVERED_OUTLINE_COLOR;
          const lineMat = new THREE.LineBasicMaterial({
            color,
            linewidth: 1,
            transparent: true,
            opacity: isSelected ? 1.0 : 0.5,
          });
          const outline = new THREE.LineSegments(edgesGeo, lineMat);
          outline.userData.isOutline = true;
          outline.raycast = () => {}; // Don't interfere with raycasting
          child.add(outline);
        }
      }
    });
  }

  // ─── Public API: Raycasting ──────────────────────

  /**
   * Raycast from a pointer event and return the ObjectId of the first hit, or null.
   */
  raycastObjects(event: PointerEvent): ObjectId | null {
    this.updateNdc(event);
    this.raycaster.setFromCamera(this.ndcMouse, this.camera);

    // Collect all meshes from object groups
    const meshes: THREE.Mesh[] = [];
    for (const [, group] of this.objectMeshes) {
      group.traverse((child) => {
        if (child instanceof THREE.Mesh) meshes.push(child);
      });
    }

    const intersects = this.raycaster.intersectObjects(meshes, false);
    if (intersects.length === 0) return null;

    // Walk up to find the group with objectId
    let obj: THREE.Object3D | null = intersects[0].object;
    while (obj) {
      if (obj.userData.objectId) return obj.userData.objectId as ObjectId;
      obj = obj.parent;
    }
    return null;
  }

  /**
   * Raycast to the ground plane (Y=0 in Three.js) and return grid-snapped CadQuery position.
   */
  getGridIntersection(event: PointerEvent): [number, number, number] | null {
    this.updateNdc(event);
    this.raycaster.setFromCamera(this.ndcMouse, this.camera);

    const groundPlane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
    const intersection = new THREE.Vector3();
    const hit = this.raycaster.ray.intersectPlane(groundPlane, intersection);

    if (!hit) return null;

    // Snap to grid
    const cs = this.gridCellSize;
    const snapped = new THREE.Vector3(
      Math.round(intersection.x / cs) * cs,
      0,
      Math.round(intersection.z / cs) * cs,
    );

    // Convert to CadQuery coords: Three.js (x, 0, z) -> CadQuery (x, -z, 0)
    return [snapped.x, -snapped.z, 0];
  }

  private updateNdc(event: PointerEvent): void {
    const rect = this.container.getBoundingClientRect();
    this.ndcMouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.ndcMouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
  }

  // ─── Public API: View Controls ───────────────────

  /**
   * Animate camera to a standard view preset.
   */
  animateToView(view: 'top' | 'front' | 'right' | 'iso'): void {
    const target = this.controls.target.clone();
    const distance = this.camera.position.distanceTo(target);

    let direction: THREE.Vector3;
    switch (view) {
      case 'top':
        direction = new THREE.Vector3(0, 1, 0);
        break;
      case 'front':
        direction = new THREE.Vector3(0, 0, 1);
        break;
      case 'right':
        direction = new THREE.Vector3(1, 0, 0);
        break;
      case 'iso':
        direction = new THREE.Vector3(1, 0.75, 1).normalize();
        break;
    }

    const targetPos = target.clone().add(direction.multiplyScalar(distance));
    this.animateCameraTo(targetPos, target);
  }

  /**
   * Fit all objects in view with smooth animation.
   */
  fitAll(): void {
    const box = new THREE.Box3();
    let hasContent = false;

    // Include all parametric objects
    for (const [, group] of this.objectMeshes) {
      box.expandByObject(group);
      hasContent = true;
    }

    // Include legacy current model
    if (this.currentModel) {
      box.expandByObject(this.currentModel);
      hasContent = true;
    }

    if (!hasContent) {
      // Empty scene: reset to default view
      this.animateCameraTo(
        new THREE.Vector3(8, 6, 8),
        new THREE.Vector3(0, 0, 0),
      );
      return;
    }

    const center = new THREE.Vector3();
    const size = new THREE.Vector3();
    box.getCenter(center);
    box.getSize(size);

    const maxDim = Math.max(size.x, size.y, size.z);
    const fov = this.camera.fov * (Math.PI / 180);
    let cameraDistance = maxDim / (2 * Math.tan(fov / 2));
    cameraDistance *= 1.8; // padding

    const direction = new THREE.Vector3(1, 0.75, 1).normalize();
    const targetPos = center.clone().add(direction.multiplyScalar(cameraDistance));

    this.animateCameraTo(targetPos, center);
  }

  /**
   * Toggle grid visibility.
   */
  setGridVisible(visible: boolean): void {
    this.grid.visible = visible;
  }

  /**
   * Toggle axes helper visibility.
   */
  setAxesVisible(visible: boolean): void {
    this.axes.visible = visible;
  }

  /**
   * Rebuild the grid with new size and spacing.
   */
  rebuildGrid(size: number, spacing: number, majorColor = 0x404060, minorColor = 0x2a2a40): void {
    this.scene.remove(this.grid);
    this.grid.geometry.dispose();
    if (Array.isArray(this.grid.material)) {
      this.grid.material.forEach((m) => m.dispose());
    } else {
      (this.grid.material as THREE.Material).dispose();
    }

    const divisions = Math.round(size / spacing);
    this.grid = new THREE.GridHelper(size, divisions, majorColor, minorColor);
    this.scene.add(this.grid);
    this.gridCellSize = spacing;
    this.gridSize = size;
  }

  /**
   * Update scene colors based on theme.
   */
  setThemeColors(theme: 'dark' | 'light'): void {
    if (theme === 'light') {
      this.scene.background = new THREE.Color(0xdce0e8);
      this.hemisphereLight.groundColor.setHex(0xdce0e8);
      this.rebuildGrid(this.gridSize, this.gridCellSize, 0x9ca0b0, 0xbcc0cc);
    } else {
      this.scene.background = new THREE.Color(0x1a1a2e);
      this.hemisphereLight.groundColor.setHex(0x1a1a2e);
      this.rebuildGrid(this.gridSize, this.gridCellSize, 0x404060, 0x2a2a40);
    }
  }

  /**
   * Smoothly animate camera to a target position and look-at point.
   */
  private animateCameraTo(
    targetPos: THREE.Vector3,
    targetLookAt: THREE.Vector3,
    duration = 400,
  ): void {
    if (this._isAnimatingView) return;
    this._isAnimatingView = true;

    const startPos = this.camera.position.clone();
    const startTarget = this.controls.target.clone();
    const startTime = performance.now();

    // Temporarily disable damping to prevent OrbitControls from fighting
    const wasDamping = this.controls.enableDamping;
    this.controls.enableDamping = false;

    const step = () => {
      const elapsed = performance.now() - startTime;
      const t = Math.min(elapsed / duration, 1);
      // Ease-in-out cubic
      const ease = t < 0.5
        ? 4 * t * t * t
        : 1 - Math.pow(-2 * t + 2, 3) / 2;

      this.camera.position.lerpVectors(startPos, targetPos, ease);
      this.controls.target.lerpVectors(startTarget, targetLookAt, ease);
      this.controls.update();

      if (t < 1) {
        requestAnimationFrame(step);
      } else {
        this.controls.enableDamping = wasDamping;
        this._isAnimatingView = false;
      }
    };

    requestAnimationFrame(step);
  }

  // ─── Public API: Placement Ghost ─────────────────

  /**
   * Show a semi-transparent ghost mesh for placement preview.
   */
  showPlacementGhost(type: PrimitiveType): void {
    this.clearGhost();

    const params = getDefaultParams(type);
    const geometry = this.createGeometry(params);
    const material = new THREE.MeshStandardMaterial({
      color: DEFAULT_COLOR,
      transparent: true,
      opacity: 0.4,
      depthWrite: false,
    });
    const mesh = new THREE.Mesh(geometry, material);

    const group = new THREE.Group();
    group.add(mesh);
    // Start off-screen until first mouse move
    group.visible = false;

    this.ghostMesh = group;
    this.scene.add(group);
  }

  /**
   * Move the ghost to a CadQuery position (grid-snapped).
   */
  updateGhostPosition(cadPos: [number, number, number]): void {
    if (!this.ghostMesh) return;

    const pos = cadToThreePos(cadPos);
    this.ghostMesh.position.copy(pos);
    this.ghostMesh.visible = true;
  }

  /**
   * Remove and dispose the ghost mesh.
   */
  clearGhost(): void {
    if (!this.ghostMesh) return;

    this.scene.remove(this.ghostMesh);
    this.ghostMesh.traverse((child) => {
      if (child instanceof THREE.Mesh) {
        child.geometry.dispose();
        if (Array.isArray(child.material)) {
          child.material.forEach((m) => m.dispose());
        } else {
          child.material.dispose();
        }
      }
    });
    this.ghostMesh = null;
  }

  // ─── Public API: Sketch Mode ─────────────────────

  /**
   * Enter sketch mode: initialize renderer, animate camera to face the plane.
   */
  enterSketchMode(sketch: Sketch): void {
    this.sketchRenderer.enterSketch(sketch);

    // Animate camera to face the sketch plane
    const planeInfo = getSketchPlaneInfo(sketch.plane, sketch.origin);
    const { position, target } = getSketchViewCamera(planeInfo);
    this.animateCameraTo(position, target);
  }

  /**
   * Exit sketch mode: cleanup renderer.
   */
  exitSketchMode(): void {
    this.sketchRenderer.exitSketch();
  }

  /**
   * Sync sketch entity rendering.
   */
  syncSketchEntities(sketch: Sketch, selectedIds: SketchEntityId[], hoveredId: SketchEntityId | null, cState: ConstraintState = 'under-constrained'): void {
    this.sketchRenderer.syncEntities(sketch, selectedIds, hoveredId, cState);
  }

  /**
   * Sync sketch constraint rendering.
   */
  syncSketchConstraints(sketch: Sketch, cState: ConstraintState): void {
    this.sketchRenderer.syncConstraints(sketch, cState);
  }

  /**
   * Update sketch preview (rubber-band).
   */
  updateSketchPreview(tool: SketchToolId, points: Point2D[], previewPoint: Point2D | null, sketch: Sketch): void {
    this.sketchRenderer.updatePreview(tool, points, previewPoint, sketch);
  }

  /**
   * Clear sketch preview.
   */
  clearSketchPreview(): void {
    this.sketchRenderer.clearPreview();
  }

  /**
   * Render all non-active sketches as static lines.
   */
  syncInactiveSketches(sketches: Sketch[], activeSketchId: SketchId | null): void {
    this.sketchRenderer.syncInactiveSketches(sketches, activeSketchId);
  }

  /**
   * Raycast to the sketch plane, returning the 2D sketch coordinate.
   */
  getSketchPlaneIntersection(event: PointerEvent, sketch: Sketch): Point2D | null {
    this.updateNdc(event);
    this.raycaster.setFromCamera(this.ndcMouse, this.camera);

    const planeInfo = getSketchPlaneInfo(sketch.plane, sketch.origin);
    const intersection = new THREE.Vector3();
    const hit = this.raycaster.ray.intersectPlane(planeInfo.plane, intersection);
    if (!hit) return null;

    return threeToSketchPos(intersection, planeInfo);
  }

  /**
   * Raycast against inactive (finished) sketch line geometry to select sketches in 3D mode.
   * Returns the SketchId of the first hit, or null.
   */
  raycastInactiveSketches(event: PointerEvent): SketchId | null {
    this.updateNdc(event);
    this.raycaster.setFromCamera(this.ndcMouse, this.camera);

    const inactiveMeshes = this.sketchRenderer.getInactiveMeshes();
    const allLines: THREE.Line[] = [];
    const lineToSketchId = new Map<THREE.Line, SketchId>();

    for (const [sketchId, group] of inactiveMeshes) {
      group.traverse((child) => {
        if (child instanceof THREE.Line) {
          allLines.push(child);
          lineToSketchId.set(child, sketchId);
        }
      });
    }

    if (allLines.length === 0) return null;

    // Force world matrix update so raycaster has correct transforms
    for (const line of allLines) {
      line.updateMatrixWorld(true);
    }

    // Use a generous threshold for line raycasting
    const oldThreshold = this.raycaster.params.Line?.threshold ?? 0.1;
    if (!this.raycaster.params.Line) this.raycaster.params.Line = { threshold: 2.0 };
    else this.raycaster.params.Line.threshold = 2.0;

    const intersects = this.raycaster.intersectObjects(allLines, false);

    this.raycaster.params.Line!.threshold = oldThreshold;

    if (intersects.length === 0) return null;

    return lineToSketchId.get(intersects[0].object as THREE.Line) ?? null;
  }

  /**
   * Highlight an inactive sketch by changing its line color.
   */
  highlightInactiveSketch(id: SketchId | null): void {
    this.sketchRenderer.highlightInactiveSketch(id);
  }

  /**
   * Hit-test sketch entities at the given pointer event position.
   */
  raycastSketchEntities(event: PointerEvent, sketch: Sketch, threshold = 0.5): SketchEntityId | null {
    this.updateNdc(event);
    this.raycaster.setFromCamera(this.ndcMouse, this.camera);

    const planeInfo = getSketchPlaneInfo(sketch.plane, sketch.origin);
    const intersection = new THREE.Vector3();
    const hit = this.raycaster.ray.intersectPlane(planeInfo.plane, intersection);
    if (!hit) return null;

    return this.sketchRenderer.raycastSketchEntities(intersection, sketch, threshold);
  }

  // ─── Public API: Datum Geometry ─────────────────

  /**
   * Add a datum plane visualization (semi-transparent quad with border).
   */
  addDatumPlane(
    id: DatumId,
    origin: THREE.Vector3,
    normal: THREE.Vector3,
    u: THREE.Vector3,
    v: THREE.Vector3,
    color: string,
    size = 20,
  ): void {
    this.removeDatum(id);

    const group = new THREE.Group();
    group.userData.datumId = id;

    const halfSize = size / 2;
    const threeColor = new THREE.Color(color);

    // Semi-transparent quad
    const planeGeo = new THREE.PlaneGeometry(size, size);
    const planeMat = new THREE.MeshBasicMaterial({
      color: threeColor,
      transparent: true,
      opacity: 0.12,
      side: THREE.DoubleSide,
      depthWrite: false,
    });
    const planeMesh = new THREE.Mesh(planeGeo, planeMat);

    // Orient the plane to match the datum basis vectors
    const m = new THREE.Matrix4();
    m.makeBasis(u, v, normal);
    planeMesh.quaternion.setFromRotationMatrix(m);
    planeMesh.position.copy(origin);

    group.add(planeMesh);

    // Border lines
    const corners = [
      new THREE.Vector3().copy(origin).addScaledVector(u, -halfSize).addScaledVector(v, -halfSize),
      new THREE.Vector3().copy(origin).addScaledVector(u, halfSize).addScaledVector(v, -halfSize),
      new THREE.Vector3().copy(origin).addScaledVector(u, halfSize).addScaledVector(v, halfSize),
      new THREE.Vector3().copy(origin).addScaledVector(u, -halfSize).addScaledVector(v, halfSize),
      new THREE.Vector3().copy(origin).addScaledVector(u, -halfSize).addScaledVector(v, -halfSize),
    ];
    const lineGeo = new THREE.BufferGeometry().setFromPoints(corners);
    const lineMat = new THREE.LineBasicMaterial({
      color: threeColor,
      transparent: true,
      opacity: 0.5,
    });
    const borderLine = new THREE.Line(lineGeo, lineMat);
    group.add(borderLine);

    this.datumMeshes.set(id, group);
    this.scene.add(group);
  }

  /**
   * Add a datum axis visualization (arrow in both directions).
   */
  addDatumAxis(
    id: DatumId,
    origin: THREE.Vector3,
    direction: THREE.Vector3,
    color: string,
    length = 30,
  ): void {
    this.removeDatum(id);

    const group = new THREE.Group();
    group.userData.datumId = id;

    const threeColor = new THREE.Color(color);
    const halfLength = length / 2;

    // Forward arrow
    const arrowPos = new THREE.ArrowHelper(
      direction.clone().normalize(),
      origin.clone().addScaledVector(direction, -halfLength),
      length,
      threeColor,
      length * 0.06,
      length * 0.03,
    );
    group.add(arrowPos);

    this.datumMeshes.set(id, group);
    this.scene.add(group);
  }

  /**
   * Remove a datum visualization from the scene.
   */
  removeDatum(id: DatumId): void {
    const group = this.datumMeshes.get(id);
    if (!group) return;

    this.scene.remove(group);
    group.traverse((child) => {
      if (child instanceof THREE.Mesh || child instanceof THREE.LineSegments || child instanceof THREE.Line) {
        child.geometry.dispose();
        if (Array.isArray(child.material)) {
          child.material.forEach((m) => m.dispose());
        } else {
          (child.material as THREE.Material).dispose();
        }
      }
    });
    this.datumMeshes.delete(id);
  }

  /**
   * Remove all datum visualizations.
   */
  removeAllDatums(): void {
    for (const id of [...this.datumMeshes.keys()]) {
      this.removeDatum(id);
    }
  }

  /**
   * Toggle visibility of a datum visualization.
   */
  setDatumVisible(id: DatumId, visible: boolean): void {
    const group = this.datumMeshes.get(id);
    if (group) group.visible = visible;
  }

  // ─── Public API: Measurements ──────────────────

  /**
   * Enhanced raycast returning world-space hit point, object ID, and face normal.
   */
  raycastSurface(event: PointerEvent): { point: THREE.Vector3; objectId: ObjectId | null; normal: THREE.Vector3 | null } | null {
    this.updateNdc(event);
    this.raycaster.setFromCamera(this.ndcMouse, this.camera);

    const meshes: THREE.Mesh[] = [];
    for (const [, group] of this.objectMeshes) {
      group.traverse((child) => {
        if (child instanceof THREE.Mesh && !child.userData.isOutline) meshes.push(child);
      });
    }

    const intersects = this.raycaster.intersectObjects(meshes, false);
    if (intersects.length === 0) return null;

    const hit = intersects[0];
    const point = hit.point.clone();
    const normal = hit.face ? hit.face.normal.clone().transformDirection(hit.object.matrixWorld) : null;

    let objectId: ObjectId | null = null;
    let obj: THREE.Object3D | null = hit.object;
    while (obj) {
      if (obj.userData.objectId) { objectId = obj.userData.objectId as ObjectId; break; }
      obj = obj.parent;
    }

    return { point, objectId, normal };
  }

  /**
   * Add a distance measurement overlay.
   */
  addDistanceMeasurement(id: MeasurementId, p1: THREE.Vector3, p2: THREE.Vector3, distance: number): void {
    this.removeMeasurement(id);

    const group = new THREE.Group();
    group.userData.measurementId = id;

    const TEAL = 0x94e2d5;

    // Dashed line between points
    const lineGeo = new THREE.BufferGeometry().setFromPoints([p1, p2]);
    const lineMat = new THREE.LineDashedMaterial({
      color: TEAL,
      dashSize: 0.3,
      gapSize: 0.15,
      transparent: true,
      opacity: 0.9,
      depthTest: false,
      depthWrite: false,
    });
    const line = new THREE.Line(lineGeo, lineMat);
    line.computeLineDistances();
    line.renderOrder = 10;
    group.add(line);

    // Endpoint markers
    const markerGeo = new THREE.SphereGeometry(0.12, 8, 8);
    const markerMat = new THREE.MeshBasicMaterial({ color: TEAL, depthTest: false, depthWrite: false });
    const marker1 = new THREE.Mesh(markerGeo, markerMat);
    marker1.position.copy(p1);
    marker1.renderOrder = 11;
    group.add(marker1);

    const marker2 = new THREE.Mesh(markerGeo.clone(), markerMat.clone());
    marker2.position.copy(p2);
    marker2.renderOrder = 11;
    group.add(marker2);

    // Label at midpoint
    const mid = new THREE.Vector3().lerpVectors(p1, p2, 0.5);
    mid.y += 0.4; // slight offset above the line
    const label = this.makeTextSprite(`${distance.toFixed(2)}`, mid, '#94e2d5', 1.2);
    group.add(label);

    this.measurementMeshes.set(id, group);
    this.measurementGroup.add(group);
  }

  /**
   * Add an angle measurement overlay.
   */
  addAngleMeasurement(id: MeasurementId, vertex: THREE.Vector3, arm1Pos: THREE.Vector3, arm2Pos: THREE.Vector3, angleDeg: number): void {
    this.removeMeasurement(id);

    const group = new THREE.Group();
    group.userData.measurementId = id;

    const TEAL = 0x94e2d5;

    // Short arm lines from vertex
    const armLen = 2.0;
    const dir1 = new THREE.Vector3().subVectors(arm1Pos, vertex).normalize();
    const dir2 = new THREE.Vector3().subVectors(arm2Pos, vertex).normalize();
    const end1 = vertex.clone().addScaledVector(dir1, armLen);
    const end2 = vertex.clone().addScaledVector(dir2, armLen);

    const arm1Geo = new THREE.BufferGeometry().setFromPoints([vertex, end1]);
    const arm2Geo = new THREE.BufferGeometry().setFromPoints([vertex, end2]);
    const armMat = new THREE.LineBasicMaterial({
      color: TEAL,
      transparent: true,
      opacity: 0.9,
      depthTest: false,
      depthWrite: false,
    });
    const armLine1 = new THREE.Line(arm1Geo, armMat);
    armLine1.renderOrder = 10;
    group.add(armLine1);
    const armLine2 = new THREE.Line(arm2Geo, armMat.clone());
    armLine2.renderOrder = 10;
    group.add(armLine2);

    // Arc between arms
    const arcRadius = 1.2;
    const arcPoints: THREE.Vector3[] = [];
    const angleRad = angleDeg * (Math.PI / 180);
    const segments = 24;

    // Build rotation basis: from dir1 toward dir2
    const normal = new THREE.Vector3().crossVectors(dir1, dir2).normalize();
    if (normal.length() < 0.001) {
      // Degenerate (parallel), skip arc
    } else {
      for (let i = 0; i <= segments; i++) {
        const t = (i / segments) * angleRad;
        const p = dir1.clone().applyAxisAngle(normal, t).multiplyScalar(arcRadius).add(vertex);
        arcPoints.push(p);
      }
      const arcGeo = new THREE.BufferGeometry().setFromPoints(arcPoints);
      const arcLine = new THREE.Line(arcGeo, armMat.clone());
      arcLine.renderOrder = 10;
      group.add(arcLine);
    }

    // Degree label
    const labelDir = dir1.clone().add(dir2).normalize();
    if (labelDir.length() < 0.001) labelDir.copy(dir1);
    const labelPos = vertex.clone().addScaledVector(labelDir, arcRadius + 0.5);
    const label = this.makeTextSprite(`${angleDeg.toFixed(1)}\u00B0`, labelPos, '#94e2d5', 1.0);
    group.add(label);

    // Vertex marker
    const markerGeo = new THREE.SphereGeometry(0.12, 8, 8);
    const markerMat = new THREE.MeshBasicMaterial({ color: TEAL, depthTest: false, depthWrite: false });
    const marker = new THREE.Mesh(markerGeo, markerMat);
    marker.position.copy(vertex);
    marker.renderOrder = 11;
    group.add(marker);

    this.measurementMeshes.set(id, group);
    this.measurementGroup.add(group);
  }

  /**
   * Add a radius measurement overlay.
   */
  addRadiusMeasurement(id: MeasurementId, center: THREE.Vector3, radius: number): void {
    this.removeMeasurement(id);

    const group = new THREE.Group();
    group.userData.measurementId = id;

    const TEAL = 0x94e2d5;

    // Dashed line from center outward (along X)
    const endPoint = center.clone().add(new THREE.Vector3(radius, 0, 0));
    const lineGeo = new THREE.BufferGeometry().setFromPoints([center, endPoint]);
    const lineMat = new THREE.LineDashedMaterial({
      color: TEAL,
      dashSize: 0.2,
      gapSize: 0.1,
      transparent: true,
      opacity: 0.9,
      depthTest: false,
      depthWrite: false,
    });
    const line = new THREE.Line(lineGeo, lineMat);
    line.computeLineDistances();
    line.renderOrder = 10;
    group.add(line);

    // Center marker
    const markerGeo = new THREE.SphereGeometry(0.12, 8, 8);
    const markerMat = new THREE.MeshBasicMaterial({ color: TEAL, depthTest: false, depthWrite: false });
    const marker = new THREE.Mesh(markerGeo, markerMat);
    marker.position.copy(center);
    marker.renderOrder = 11;
    group.add(marker);

    // Label
    const labelPos = new THREE.Vector3().lerpVectors(center, endPoint, 0.5);
    labelPos.y += 0.4;
    const label = this.makeTextSprite(`R=${radius.toFixed(2)}`, labelPos, '#94e2d5', 1.0);
    group.add(label);

    this.measurementMeshes.set(id, group);
    this.measurementGroup.add(group);
  }

  /**
   * Add a bounding box measurement overlay.
   */
  addBBoxMeasurement(id: MeasurementId, objectId: ObjectId): void {
    this.removeMeasurement(id);

    const objGroup = this.objectMeshes.get(objectId);
    if (!objGroup) return;

    const group = new THREE.Group();
    group.userData.measurementId = id;

    const TEAL = 0x94e2d5;

    // Compute bounding box
    const box = new THREE.Box3().setFromObject(objGroup);
    const min = box.min;
    const max = box.max;
    const size = new THREE.Vector3();
    box.getSize(size);

    // Wireframe box
    const boxGeo = new THREE.BoxGeometry(size.x, size.y, size.z);
    const center = new THREE.Vector3();
    box.getCenter(center);
    const edgesGeo = new THREE.EdgesGeometry(boxGeo);
    const edgesMat = new THREE.LineBasicMaterial({
      color: TEAL,
      transparent: true,
      opacity: 0.6,
      depthTest: false,
      depthWrite: false,
    });
    const edges = new THREE.LineSegments(edgesGeo, edgesMat);
    edges.position.copy(center);
    edges.renderOrder = 10;
    group.add(edges);

    // Dimension labels at edge midpoints
    const xLabel = this.makeTextSprite(
      `X: ${size.x.toFixed(2)}`,
      new THREE.Vector3(center.x, min.y - 0.5, center.z),
      '#94e2d5',
      1.0,
    );
    group.add(xLabel);

    const yLabel = this.makeTextSprite(
      `Y: ${size.y.toFixed(2)}`,
      new THREE.Vector3(max.x + 0.5, center.y, center.z),
      '#94e2d5',
      1.0,
    );
    group.add(yLabel);

    const zLabel = this.makeTextSprite(
      `Z: ${size.z.toFixed(2)}`,
      new THREE.Vector3(center.x, min.y - 0.5, max.z + 0.5),
      '#94e2d5',
      1.0,
    );
    group.add(zLabel);

    this.measurementMeshes.set(id, group);
    this.measurementGroup.add(group);
  }

  /**
   * Remove a single measurement overlay.
   */
  removeMeasurement(id: MeasurementId): void {
    const group = this.measurementMeshes.get(id);
    if (!group) return;

    this.measurementGroup.remove(group);
    group.traverse((child) => {
      if (child instanceof THREE.Mesh || child instanceof THREE.LineSegments || child instanceof THREE.Line) {
        child.geometry.dispose();
        if (Array.isArray(child.material)) {
          child.material.forEach((m) => m.dispose());
        } else {
          (child.material as THREE.Material).dispose();
        }
      }
      if (child instanceof THREE.Sprite) {
        child.material.map?.dispose();
        child.material.dispose();
      }
    });
    this.measurementMeshes.delete(id);
  }

  /**
   * Remove all measurement overlays.
   */
  clearAllMeasurements(): void {
    for (const id of [...this.measurementMeshes.keys()]) {
      this.removeMeasurement(id);
    }
  }

  /**
   * Show a pending marker dot at the given position.
   */
  showPendingMarker(pos: THREE.Vector3): void {
    this.clearPendingMarker();
    const geo = new THREE.SphereGeometry(0.18, 12, 12);
    const mat = new THREE.MeshBasicMaterial({
      color: 0x94e2d5,
      transparent: true,
      opacity: 0.8,
      depthTest: false,
      depthWrite: false,
    });
    this.pendingMarker = new THREE.Mesh(geo, mat);
    this.pendingMarker.position.copy(pos);
    this.pendingMarker.renderOrder = 12;
    this.measurementGroup.add(this.pendingMarker);
  }

  /**
   * Remove the pending marker.
   */
  clearPendingMarker(): void {
    if (!this.pendingMarker) return;
    this.measurementGroup.remove(this.pendingMarker);
    this.pendingMarker.geometry.dispose();
    (this.pendingMarker.material as THREE.Material).dispose();
    this.pendingMarker = null;
  }

  /**
   * Get the set of measurement IDs currently rendered in the engine.
   */
  getMeasurementIds(): Set<MeasurementId> {
    return new Set(this.measurementMeshes.keys());
  }

  /**
   * Get the bounding box of an object mesh.
   */
  getObjectBBox(objectId: ObjectId): THREE.Box3 | null {
    const group = this.objectMeshes.get(objectId);
    if (!group) return null;
    return new THREE.Box3().setFromObject(group);
  }

  private makeTextSprite(
    text: string,
    position: THREE.Vector3,
    color: string,
    scale = 1.0,
  ): THREE.Sprite {
    const canvas = document.createElement('canvas');
    const size = 256;
    canvas.width = size;
    canvas.height = 64;
    const ctx = canvas.getContext('2d')!;

    // Background pill
    ctx.fillStyle = 'rgba(30, 30, 46, 0.9)';
    ctx.beginPath();
    ctx.roundRect(2, 2, size - 4, 60, 8);
    ctx.fill();

    // Border
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.roundRect(2, 2, size - 4, 60, 8);
    ctx.stroke();

    // Text
    ctx.font = 'bold 32px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = color;
    ctx.fillText(text, size / 2, 32);

    const texture = new THREE.CanvasTexture(canvas);
    texture.needsUpdate = true;
    const material = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthWrite: false,
      depthTest: false,
    });
    const sprite = new THREE.Sprite(material);
    sprite.position.copy(position);
    sprite.scale.set(scale * 2, scale * 0.5, 1);
    sprite.renderOrder = 12;
    return sprite;
  }

  // ─── Public API: Display Modes ──────────────────

  /**
   * Set the display mode for all CAD meshes.
   */
  setDisplayMode(mode: DisplayMode): void {
    this.currentDisplayMode = mode;
    this.applyDisplayModeToAll();
  }

  /**
   * Set or remove the section/clipping plane.
   */
  setSectionPlane(config: SectionPlaneConfig): void {
    // Remove existing helper
    if (this.sectionPlaneHelper) {
      this.scene.remove(this.sectionPlaneHelper);
      this.sectionPlaneHelper = null;
    }

    if (config.enabled) {
      const normal = new THREE.Vector3(...config.normal).normalize();
      // Plane equation: dot(normal, point) + constant = 0
      // We want to clip everything on the positive side of (origin + offset * normal)
      this.clippingPlane = new THREE.Plane(normal, -config.offset);
      this.renderer.clippingPlanes = [this.clippingPlane];

      // Visual helper
      this.sectionPlaneHelper = new THREE.PlaneHelper(this.clippingPlane, 30, 0xf38ba8);
      this.scene.add(this.sectionPlaneHelper);
    } else {
      this.clippingPlane = null;
      this.renderer.clippingPlanes = [];
    }

    this.renderer.localClippingEnabled = config.enabled;
  }

  private applyDisplayModeToAll(): void {
    // Apply to parametric object meshes
    for (const [id] of this.objectMeshes) {
      this.applyDisplayModeToGroup(id);
    }

    // Apply to legacy current model
    if (this.currentModel && this.currentModel instanceof THREE.Mesh) {
      this.applyDisplayModeToMesh(this.currentModel);
    }
  }

  private applyDisplayModeToGroup(id: ObjectId): void {
    const group = this.objectMeshes.get(id);
    if (!group) return;

    // Remove existing display edges (but not selection outlines)
    const toRemove: THREE.Object3D[] = [];
    group.traverse((child) => {
      if (child.userData.isDisplayEdge) toRemove.push(child);
    });
    for (const obj of toRemove) {
      obj.parent?.remove(obj);
      if (obj instanceof THREE.LineSegments) {
        obj.geometry.dispose();
        (obj.material as THREE.Material).dispose();
      }
    }

    // Apply mode to mesh children
    group.traverse((child) => {
      if (child instanceof THREE.Mesh && child.material instanceof THREE.MeshStandardMaterial) {
        this.applyDisplayModeToMesh(child);
      }
    });

    // Re-run selection/hover visuals so outlines coexist properly
    this.updateMeshVisuals(id);
  }

  private applyDisplayModeToMesh(mesh: THREE.Mesh): void {
    const mat = mesh.material;
    if (!(mat instanceof THREE.MeshStandardMaterial)) return;

    const mode = this.currentDisplayMode;

    const baseOpacity: number = mesh.userData.baseOpacity ?? 1.0;

    switch (mode) {
      case 'shaded': {
        mat.wireframe = false;
        mat.transparent = baseOpacity < 1;
        mat.opacity = baseOpacity;
        mat.depthWrite = baseOpacity >= 1;
        mesh.castShadow = true;
        break;
      }

      case 'wireframe':
        mat.wireframe = true;
        mat.transparent = false;
        mat.opacity = 1;
        mat.depthWrite = true;
        mesh.castShadow = false;
        break;

      case 'shaded-edges': {
        mat.wireframe = false;
        mat.transparent = baseOpacity < 1;
        mat.opacity = baseOpacity;
        mat.depthWrite = baseOpacity >= 1;
        mesh.castShadow = true;
        this.addDisplayEdges(mesh, 0x606080, 0.6);
        break;
      }

      case 'transparent':
        mat.wireframe = false;
        mat.transparent = true;
        mat.opacity = 0.3;
        mat.depthWrite = false;
        mesh.castShadow = false;
        this.addDisplayEdges(mesh, 0x89b4fa, 0.8);
        break;

      case 'section': {
        // Same material as shaded; clipping handled by renderer
        mat.wireframe = false;
        mat.transparent = baseOpacity < 1;
        mat.opacity = baseOpacity;
        mat.depthWrite = baseOpacity >= 1;
        mesh.castShadow = true;
        break;
      }
    }

    mat.needsUpdate = true;
  }

  private addDisplayEdges(mesh: THREE.Mesh, color: number, opacity: number): void {
    const edgesGeo = new THREE.EdgesGeometry(mesh.geometry, 30);
    const lineMat = new THREE.LineBasicMaterial({
      color,
      transparent: true,
      opacity,
      linewidth: 1,
    });
    const edges = new THREE.LineSegments(edgesGeo, lineMat);
    edges.userData.isDisplayEdge = true;
    edges.raycast = () => {}; // Don't interfere with raycasting
    mesh.add(edges);
  }

  // ─── Public API: Geometry Creation ───────────────

  private createGeometry(params: PrimitiveParams): THREE.BufferGeometry {
    switch (params.type) {
      case 'box':
        return new THREE.BoxGeometry(params.width, params.height, params.depth);
      case 'cylinder':
        return new THREE.CylinderGeometry(params.radius, params.radius, params.height, 32);
      case 'sphere':
        return new THREE.SphereGeometry(params.radius, 32, 24);
      case 'cone':
        return new THREE.ConeGeometry(params.bottomRadius, params.height, 32);
    }
  }

  // ─── Legacy API (for manual mode / STL loading) ──

  /**
   * Load a demo box into the scene.
   */
  loadDemoBox(): void {
    this.clearModel();

    const geometry = new THREE.BoxGeometry(2, 2, 2);
    const material = new THREE.MeshStandardMaterial({
      color: DEFAULT_COLOR,
      metalness: 0.3,
      roughness: 0.7,
    });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.set(0, 1, 0);
    mesh.castShadow = true;
    mesh.receiveShadow = true;

    this.currentModel = mesh;
    this.scene.add(mesh);

    // Apply current display mode
    if (this.currentDisplayMode !== 'shaded') {
      this.applyDisplayModeToMesh(mesh);
    }
  }

  /**
   * Load an STL model from an ArrayBuffer.
   */
  loadSTL(data: ArrayBuffer): void {
    this.clearModel();

    const loader = new STLLoader();
    const geometry = loader.parse(data);
    geometry.rotateX(-Math.PI / 2); // CadQuery Z-up → Three.js Y-up

    // Compute vertex normals for smooth shading
    geometry.computeVertexNormals();

    const material = new THREE.MeshStandardMaterial({
      color: DEFAULT_COLOR,
      metalness: 0.3,
      roughness: 0.7,
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.castShadow = true;
    mesh.receiveShadow = true;

    // Center the model
    geometry.computeBoundingBox();
    const boundingBox = geometry.boundingBox!;
    const center = new THREE.Vector3();
    boundingBox.getCenter(center);
    geometry.translate(-center.x, -center.y, -center.z);

    // Lift model so it sits on the grid (bottom at y=0)
    const size = new THREE.Vector3();
    boundingBox.getSize(size);
    mesh.position.set(0, size.y / 2, 0);

    this.currentModel = mesh;
    this.scene.add(mesh);

    // Apply current display mode
    if (this.currentDisplayMode !== 'shaded') {
      this.applyDisplayModeToMesh(mesh);
    }

    // Auto-zoom: fit the model in view
    this.fitCameraToModel(size);
  }

  /**
   * Load an STL model from a base64-encoded string.
   */
  loadSTLFromBase64(base64: string): void {
    const binaryString = atob(base64);
    const bytes = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
      bytes[i] = binaryString.charCodeAt(i);
    }
    this.loadSTL(bytes.buffer as ArrayBuffer);
  }

  /**
   * Fit the camera to frame the model nicely.
   */
  private fitCameraToModel(size: THREE.Vector3): void {
    const maxDim = Math.max(size.x, size.y, size.z);
    const fov = this.camera.fov * (Math.PI / 180);
    let cameraDistance = maxDim / (2 * Math.tan(fov / 2));
    cameraDistance *= 1.8; // Add some padding

    const direction = new THREE.Vector3(1, 0.75, 1).normalize();
    this.camera.position.copy(direction.multiplyScalar(cameraDistance));
    this.camera.lookAt(0, 0, 0);

    this.controls.target.set(0, 0, 0);
    this.controls.update();
  }

  /**
   * Remove the legacy current model from the scene.
   */
  clearModel(): void {
    if (this.currentModel) {
      this.scene.remove(this.currentModel);

      if (this.currentModel instanceof THREE.Mesh) {
        this.currentModel.geometry.dispose();
        if (Array.isArray(this.currentModel.material)) {
          this.currentModel.material.forEach((m) => m.dispose());
        } else {
          this.currentModel.material.dispose();
        }
      }

      this.currentModel = null;
    }
  }

  // ─── Core ────────────────────────────────────────

  /**
   * Handle container resize.
   */
  resize(): void {
    const { clientWidth: w, clientHeight: h } = this.container;
    if (w === 0 || h === 0) return;

    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  // ── Explosion offsets ──
  private explosionOffsets: Map<string, THREE.Vector3> = new Map();

  applyExplosion(offsets: Map<string, THREE.Vector3>): void {
    // First, reverse any existing offsets
    this.clearExplosion();

    // Apply new offsets
    for (const [objectId, offset] of offsets) {
      const group = this.objectMeshes.get(objectId);
      if (group) {
        group.position.add(offset);
        this.explosionOffsets.set(objectId, offset.clone());
      }
    }
  }

  clearExplosion(): void {
    for (const [objectId, offset] of this.explosionOffsets) {
      const group = this.objectMeshes.get(objectId);
      if (group) {
        group.position.sub(offset);
      }
    }
    this.explosionOffsets.clear();
  }

  /**
   * Dispose all resources.
   */
  dispose(): void {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }

    this.resizeObserver.disconnect();
    this.transformControls.detach();
    this.transformControls.dispose();
    this.viewHelper.dispose();
    this.controls.dispose();
    this.clearGhost();
    this.clearAllMeasurements();
    this.clearPendingMarker();
    this.sketchRenderer.dispose();
    this.removeAllDatums();
    this.removeAllObjects();
    this.clearModel();

    // Clean up section plane
    if (this.sectionPlaneHelper) {
      this.scene.remove(this.sectionPlaneHelper);
      this.sectionPlaneHelper = null;
    }
    this.renderer.clippingPlanes = [];
    this.renderer.localClippingEnabled = false;

    this.renderer.dispose();

    if (this.renderer.domElement.parentElement) {
      this.renderer.domElement.parentElement.removeChild(this.renderer.domElement);
    }
  }
}
