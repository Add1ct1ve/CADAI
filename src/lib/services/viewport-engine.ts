import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TransformControls } from 'three/addons/controls/TransformControls.js';
import { STLLoader } from 'three/addons/loaders/STLLoader.js';
import type { ObjectId, PrimitiveParams, PrimitiveType, CadTransform } from '$lib/types/cad';
import { getDefaultParams } from '$lib/types/cad';
import { cadToThreePos, cadToThreeRot } from '$lib/services/coord-utils';

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

  // Legacy single model (for manual mode / STL loading)
  private currentModel: THREE.Mesh | THREE.Group | null = null;

  // CAD tool object meshes
  private objectMeshes: Map<ObjectId, THREE.Group> = new Map();
  private selectedIds: Set<ObjectId> = new Set();
  private hoveredIdInternal: ObjectId | null = null;

  // Placement preview ghost
  private ghostMesh: THREE.Group | null = null;

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
    const grid = new THREE.GridHelper(100, 100, 0x404060, 0x2a2a40);
    this.scene.add(grid);

    // Axes
    const axes = new THREE.AxesHelper(5);
    this.scene.add(axes);

    // Lighting
    const ambientLight = new THREE.AmbientLight(0xffffff, 0.4);
    this.scene.add(ambientLight);

    const directionalLight = new THREE.DirectionalLight(0xffffff, 0.8);
    directionalLight.position.set(5, 10, 7);
    directionalLight.castShadow = true;
    directionalLight.shadow.mapSize.width = 1024;
    directionalLight.shadow.mapSize.height = 1024;
    this.scene.add(directionalLight);

    const hemisphereLight = new THREE.HemisphereLight(0x89b4fa, 0x1a1a2e, 0.3);
    this.scene.add(hemisphereLight);

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
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };

  // ─── Public API: Camera ──────────────────────────

  getCamera(): THREE.PerspectiveCamera {
    return this.camera;
  }

  getContainer(): HTMLElement {
    return this.container;
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
  ): void {
    // Remove existing if updating
    this.removeObject(id);

    const geometry = this.createGeometry(params);
    const material = new THREE.MeshStandardMaterial({
      color: new THREE.Color(color),
      metalness: 0.3,
      roughness: 0.7,
    });
    const mesh = new THREE.Mesh(geometry, material);
    mesh.castShadow = true;
    mesh.receiveShadow = true;

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

    // Snap to grid (1 unit)
    const snapped = new THREE.Vector3(
      Math.round(intersection.x),
      0,
      Math.round(intersection.z),
    );

    // Convert to CadQuery coords: Three.js (x, 0, z) -> CadQuery (x, -z, 0)
    return [snapped.x, -snapped.z, 0];
  }

  private updateNdc(event: PointerEvent): void {
    const rect = this.container.getBoundingClientRect();
    this.ndcMouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.ndcMouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
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
    this.controls.dispose();
    this.clearGhost();
    this.removeAllObjects();
    this.clearModel();
    this.renderer.dispose();

    if (this.renderer.domElement.parentElement) {
      this.renderer.domElement.parentElement.removeChild(this.renderer.domElement);
    }
  }
}
