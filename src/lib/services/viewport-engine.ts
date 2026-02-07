import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { STLLoader } from 'three/addons/loaders/STLLoader.js';

export class ViewportEngine {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private container: HTMLElement;
  private animationId: number | null = null;
  private resizeObserver: ResizeObserver;
  private currentModel: THREE.Mesh | THREE.Group | null = null;

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

    // Controls
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 2;
    this.controls.maxDistance = 100;
    this.controls.target.set(0, 0, 0);

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

  /**
   * Load a demo box into the scene
   */
  loadDemoBox(): void {
    this.clearModel();

    const geometry = new THREE.BoxGeometry(2, 2, 2);
    const material = new THREE.MeshStandardMaterial({
      color: 0x89b4fa,
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
   * Load an STL model from an ArrayBuffer
   */
  loadSTL(data: ArrayBuffer): void {
    this.clearModel();

    const loader = new STLLoader();
    const geometry = loader.parse(data);

    // Compute vertex normals for smooth shading
    geometry.computeVertexNormals();

    const material = new THREE.MeshStandardMaterial({
      color: 0x89b4fa,
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
   * Load an STL model from a base64-encoded string
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
   * Fit the camera to frame the model nicely
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
   * Remove the current model from the scene
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

  /**
   * Handle container resize
   */
  resize(): void {
    const { clientWidth: w, clientHeight: h } = this.container;
    if (w === 0 || h === 0) return;

    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  /**
   * Dispose all resources
   */
  dispose(): void {
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }

    this.resizeObserver.disconnect();
    this.controls.dispose();
    this.clearModel();
    this.renderer.dispose();

    if (this.renderer.domElement.parentElement) {
      this.renderer.domElement.parentElement.removeChild(this.renderer.domElement);
    }
  }
}
