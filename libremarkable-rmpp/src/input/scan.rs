use super::ecodes;
use super::InputDevice;
use cgmath::Vector2;
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const INITIAL_DEVS_AVAILABLE_FOR: Duration = Duration::from_millis(150);

/// A singleton of the EvDevsScan object
pub static SCANNED: LazyLock<EvDevs> = LazyLock::new(EvDevs::new);

/// This struct contains the results of initially scaning all evdev devices,
/// which allows for device model independancy.
/// Some of its data is used by other constants.
///
/// EvDevsScan has some internal mutability to allow resuing the opened devices
/// for some time to increase performance.
/// TODO: Call this `EvDevsScanOutcome` or EvScanOutcome instead ??
pub struct EvDevs {
    pub wacom_path: PathBuf,
    pub multitouch_path: PathBuf,
    pub gpio_path: PathBuf,

    pub wacom_width: u16,
    pub wacom_height: u16,
    pub mt_width: u16,
    pub mt_height: u16,

    /// The resolution of the wacom no rotation applied
    pub wacom_orig_size: Vector2<u16>,
    pub multitouch_orig_size: Vector2<u16>,

    // Those will be preserved in case they are needed fairly fast
    // to prevent any additional delay of re-opening the fds.
    // They will get removed fairly quickly though.
    wacom_initial_dev: Arc<Mutex<Option<evdev::Device>>>,
    multitouch_initial_dev: Arc<Mutex<Option<evdev::Device>>>,
    gpio_initial_dev: Arc<Mutex<Option<evdev::Device>>>,
}

struct ScanResult {
    wacom: Option<(PathBuf, evdev::Device)>,
    multitouch: Option<(PathBuf, evdev::Device)>,
    gpio: Option<(PathBuf, evdev::Device)>,
}

impl EvDevs {
    /// Open every /dev/input/event* once and classify it by capabilities.
    /// Unreadable devices are skipped instead of aborting the scan.
    fn scan_once() -> ScanResult {
        let mut result = ScanResult {
            wacom: None,
            multitouch: None,
            gpio: None,
        };
        // Whether the currently selected touchscreen advertises EV_REL (the
        // AppLoad shim's emulated touchscreen does, the real Elan panel doesn't).
        let mut multitouch_has_rel = false;

        let input_dir = Path::new("/dev/input");
        let entries = match input_dir.read_dir() {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to list {:?}: {}", input_dir, e);
                return result;
            }
        };
        let mut event_file_paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
            .map(|e| e.path())
            .collect();
        event_file_paths.sort();

        for evdev_path in event_file_paths {
            let dev = match evdev::Device::open(&evdev_path) {
                Ok(dev) => dev,
                Err(e) => {
                    debug!("Skipping {:?}: {}", evdev_path, e);
                    continue;
                }
            };
            let events = dev.supported_events();
            let has_key = |k: evdev::Key| {
                dev.supported_keys()
                    .map(|s| s.contains(k))
                    .unwrap_or(false)
            };

            if events.contains(evdev::EventType::KEY)
                && events.contains(evdev::EventType::ABSOLUTE)
                && has_key(evdev::Key::BTN_STYLUS)
            {
                if result.wacom.is_none() {
                    result.wacom = Some((evdev_path, dev));
                }
                continue;
            }

            if events.contains(evdev::EventType::KEY) && has_key(evdev::Key::KEY_POWER) {
                if result.gpio.is_none() {
                    result.gpio = Some((evdev_path, dev));
                }
                continue;
            }

            let has_mt_slot = dev
                .supported_absolute_axes()
                .map(|s| s.contains(evdev::AbsoluteAxisType::ABS_MT_SLOT))
                .unwrap_or(false);
            if has_mt_slot {
                let has_rel = events.contains(evdev::EventType::RELATIVE);
                if result.multitouch.is_none() || (has_rel && !multitouch_has_rel) {
                    multitouch_has_rel = has_rel;
                    result.multitouch = Some((evdev_path, dev));
                }
                continue;
            }
        }

        result
    }

    /// Scan all the evdev devices, figure out which is which
    /// and get some additional data for lazy constants.
    fn new() -> Self {
        // The AppLoad shim and the kernel may not have every device ready the
        // instant the app starts, so retry for a while before giving up.
        const SCAN_ATTEMPTS: u32 = 20;
        const SCAN_RETRY_DELAY: Duration = Duration::from_millis(250);

        let mut found = Self::scan_once();
        for attempt in 1..SCAN_ATTEMPTS {
            if found.multitouch.is_some() {
                break;
            }
            warn!(
                "No multitouch evdev found (attempt {}/{}), retrying...",
                attempt, SCAN_ATTEMPTS
            );
            std::thread::sleep(SCAN_RETRY_DELAY);
            found = Self::scan_once();
        }

        let ScanResult {
            wacom,
            multitouch,
            gpio,
        } = found;

        let (multitouch_path, multitouch_dev) =
            multitouch.expect("Failed to find the multitouch evdev!");
        if wacom.is_none() {
            warn!("No pen digitizer evdev found; pen input disabled.");
        }
        if gpio.is_none() {
            warn!("No button evdev found; button input disabled.");
        }

        // SIZES
        let wacom_orig_size = match &wacom {
            Some((_, dev)) => {
                let wacom_state = dev.get_abs_state().unwrap();
                Vector2 {
                    x: wacom_state[ecodes::ABS_X as usize].maximum as u16,
                    y: wacom_state[ecodes::ABS_Y as usize].maximum as u16,
                }
            }
            // Paper Pro digitizer range
            None => Vector2 { x: 11180, y: 15340 },
        };
        let (wacom_width, wacom_height) = crate::device::CURRENT_DEVICE
            .get_wacom_placement()
            .rotation
            .rotated_size(&wacom_orig_size)
            .into();

        let mt_state = multitouch_dev.get_abs_state().unwrap();
        let multitouch_orig_size = Vector2 {
            x: mt_state[ecodes::ABS_MT_POSITION_X as usize].maximum as u16,
            y: mt_state[ecodes::ABS_MT_POSITION_Y as usize].maximum as u16,
        };
        let (mt_width, mt_height) = crate::device::CURRENT_DEVICE
            .get_multitouch_placement()
            .rotation
            .rotated_size(&multitouch_orig_size)
            .into();
        info!(
            "Multitouch evdev {:?} with range {}x{}",
            multitouch_path, multitouch_orig_size.x, multitouch_orig_size.y
        );

        // Missing optional devices point to a path that fails to open as an evdev,
        // so their EvDevContext logs an error instead of panicking.
        let missing = || PathBuf::from("/dev/null");
        let (wacom_path, wacom_dev) = match wacom {
            Some((p, d)) => (p, Some(d)),
            None => (missing(), None),
        };
        let (gpio_path, gpio_dev) = match gpio {
            Some((p, d)) => (p, Some(d)),
            None => (missing(), None),
        };

        // DEVICES
        let wacom_initial_dev = Arc::new(Mutex::new(wacom_dev));
        let multitouch_initial_dev = Arc::new(Mutex::new(Some(multitouch_dev)));
        let gpio_initial_dev = Arc::new(Mutex::new(gpio_dev));
        // Spawn a thread to remove close the initial devices after some time
        let wacom_initial_dev2 = wacom_initial_dev.clone();
        let multitouch_initial_dev2 = multitouch_initial_dev.clone();
        let gpio_initial_dev2 = gpio_initial_dev.clone();
        std::thread::spawn(move || {
            std::thread::sleep(INITIAL_DEVS_AVAILABLE_FOR);
            // Remove devices (and thereby closing them)
            (*(*wacom_initial_dev2).lock().unwrap()) = None;
            (*(*multitouch_initial_dev2).lock().unwrap()) = None;
            (*(*gpio_initial_dev2).lock().unwrap()) = None;
            debug!("Closed initially opened evdev fds (if not used by now).");
        });

        Self {
            wacom_path,
            multitouch_path,
            gpio_path,

            wacom_width,
            wacom_height,

            mt_width,
            mt_height,

            multitouch_orig_size,
            wacom_orig_size,

            wacom_initial_dev,
            multitouch_initial_dev,
            gpio_initial_dev,
        }
    }

    /// Get the path to a InputDevice
    pub fn get_path(&self, device: InputDevice) -> &PathBuf {
        match device {
            InputDevice::Wacom => &self.wacom_path,
            InputDevice::Multitouch => &self.multitouch_path,
            InputDevice::GPIO => &self.gpio_path,
            InputDevice::Unknown => panic!("\"InputDevice::Unkown\" is no device!"),
        }
    }

    /// Get a ev device. If this is called early, it can get the device used for the initial scan.
    /// If an early device is returned, it might contain a few events from the past!
    pub fn get_device(&self, device: InputDevice) -> Result<evdev::Device, impl std::error::Error> {
        let dev_arc = match device {
            InputDevice::Wacom => self.wacom_initial_dev.clone(),
            InputDevice::Multitouch => self.multitouch_initial_dev.clone(),
            InputDevice::GPIO => self.gpio_initial_dev.clone(),
            InputDevice::Unknown => panic!("\"InputDevice::Unkown\" is no device!"),
        };

        let mut resuable_device = dev_arc.lock().unwrap();
        if resuable_device.is_some() {
            Ok(resuable_device.take().unwrap())
        } else {
            evdev::Device::open(self.get_path(device))
        }
    }
}
