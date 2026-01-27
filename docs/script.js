/**
 * RustXL Documentation - Interactive JavaScript
 */

document.addEventListener('DOMContentLoaded', () => {
    initNavigation();
    initTabs();
    initCopyButtons();
    initScrollAnimations();
    initSmoothScroll();
});

/**
 * Mobile Navigation Toggle
 */
function initNavigation() {
    const navToggle = document.querySelector('.nav-toggle');
    const navLinks = document.querySelector('.nav-links');

    if (!navToggle || !navLinks) return;

    navToggle.addEventListener('click', () => {
        navToggle.classList.toggle('active');
        navLinks.classList.toggle('active');
    });

    // Close mobile nav when clicking a link
    navLinks.querySelectorAll('a').forEach(link => {
        link.addEventListener('click', () => {
            navToggle.classList.remove('active');
            navLinks.classList.remove('active');
        });
    });

    // Close mobile nav when clicking outside
    document.addEventListener('click', (e) => {
        if (!navToggle.contains(e.target) && !navLinks.contains(e.target)) {
            navToggle.classList.remove('active');
            navLinks.classList.remove('active');
        }
    });
}

/**
 * Keybindings Tab Switching
 */
function initTabs() {
    const tabButtons = document.querySelectorAll('.tab-btn');
    const tabPanels = document.querySelectorAll('.tab-panel');

    if (!tabButtons.length) return;

    tabButtons.forEach(button => {
        button.addEventListener('click', () => {
            const targetTab = button.dataset.tab;

            // Update button states
            tabButtons.forEach(btn => btn.classList.remove('active'));
            button.classList.add('active');

            // Update panel visibility
            tabPanels.forEach(panel => {
                panel.classList.remove('active');
                if (panel.id === targetTab) {
                    panel.classList.add('active');
                }
            });
        });
    });
}

/**
 * Copy to Clipboard Functionality
 */
function initCopyButtons() {
    const copyButtons = document.querySelectorAll('.copy-btn');

    copyButtons.forEach(button => {
        button.addEventListener('click', async () => {
            const textToCopy = button.dataset.copy;

            if (!textToCopy) return;

            try {
                await navigator.clipboard.writeText(textToCopy);

                // Visual feedback
                button.classList.add('copied');
                const originalSVG = button.innerHTML;
                button.innerHTML = `
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                        <polyline points="20 6 9 17 4 12"/>
                    </svg>
                `;

                setTimeout(() => {
                    button.classList.remove('copied');
                    button.innerHTML = originalSVG;
                }, 2000);
            } catch (err) {
                console.error('Failed to copy:', err);
            }
        });
    });
}

/**
 * Scroll-triggered Animations
 */
function initScrollAnimations() {
    const animatedElements = document.querySelectorAll(
        '.feature-card, .install-card, .usage-card, .type-card, .showcase-item'
    );

    if (!animatedElements.length) return;

    const observer = new IntersectionObserver(
        (entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    entry.target.classList.add('visible');
                    observer.unobserve(entry.target);
                }
            });
        },
        {
            threshold: 0.1,
            rootMargin: '0px 0px -50px 0px'
        }
    );

    animatedElements.forEach(el => observer.observe(el));
}

/**
 * Smooth Scroll for Anchor Links
 */
function initSmoothScroll() {
    document.querySelectorAll('a[href^="#"]').forEach(anchor => {
        anchor.addEventListener('click', (e) => {
            const targetId = anchor.getAttribute('href');

            if (targetId === '#') return;

            const targetElement = document.querySelector(targetId);

            if (targetElement) {
                e.preventDefault();

                const navHeight = document.querySelector('.navbar').offsetHeight;
                const targetPosition = targetElement.getBoundingClientRect().top + window.pageYOffset;

                window.scrollTo({
                    top: targetPosition - navHeight - 20,
                    behavior: 'smooth'
                });

                // Update URL without jumping
                history.pushState(null, null, targetId);
            }
        });
    });
}

/**
 * Navbar Background on Scroll
 */
(function initNavbarScroll() {
    const navbar = document.querySelector('.navbar');

    if (!navbar) return;

    let lastScroll = 0;

    window.addEventListener('scroll', () => {
        const currentScroll = window.pageYOffset;

        if (currentScroll > 100) {
            navbar.style.boxShadow = '0 1px 3px rgba(0, 0, 0, 0.3)';
        } else {
            navbar.style.boxShadow = 'none';
        }

        lastScroll = currentScroll;
    }, { passive: true });
})();

/**
 * Keyboard Navigation for Tabs
 */
document.addEventListener('keydown', (e) => {
    const activeTab = document.querySelector('.tab-btn.active');

    if (!activeTab || !['ArrowLeft', 'ArrowRight'].includes(e.key)) return;

    const tabButtons = Array.from(document.querySelectorAll('.tab-btn'));
    const currentIndex = tabButtons.indexOf(activeTab);
    let newIndex;

    if (e.key === 'ArrowLeft') {
        newIndex = currentIndex === 0 ? tabButtons.length - 1 : currentIndex - 1;
    } else {
        newIndex = currentIndex === tabButtons.length - 1 ? 0 : currentIndex + 1;
    }

    tabButtons[newIndex].click();
    tabButtons[newIndex].focus();
});

/**
 * Easter egg: Konami code reveals a fun message
 */
(function initKonamiCode() {
    const konamiCode = [
        'ArrowUp', 'ArrowUp',
        'ArrowDown', 'ArrowDown',
        'ArrowLeft', 'ArrowRight',
        'ArrowLeft', 'ArrowRight',
        'KeyB', 'KeyA'
    ];
    let konamiIndex = 0;

    document.addEventListener('keydown', (e) => {
        if (e.code === konamiCode[konamiIndex]) {
            konamiIndex++;

            if (konamiIndex === konamiCode.length) {
                showEasterEgg();
                konamiIndex = 0;
            }
        } else {
            konamiIndex = 0;
        }
    });

    function showEasterEgg() {
        const message = document.createElement('div');
        message.style.cssText = `
            position: fixed;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            padding: 2rem 3rem;
            background: linear-gradient(135deg, #f97316, #ea580c);
            color: white;
            font-size: 1.5rem;
            font-weight: 600;
            border-radius: 1rem;
            box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
            z-index: 9999;
            animation: popIn 0.3s ease;
        `;
        message.textContent = 'You found the secret! Happy spreadsheeting!';

        const style = document.createElement('style');
        style.textContent = `
            @keyframes popIn {
                from { opacity: 0; transform: translate(-50%, -50%) scale(0.8); }
                to { opacity: 1; transform: translate(-50%, -50%) scale(1); }
            }
        `;
        document.head.appendChild(style);
        document.body.appendChild(message);

        setTimeout(() => {
            message.style.animation = 'popIn 0.3s ease reverse';
            setTimeout(() => message.remove(), 300);
        }, 3000);
    }
})();

/**
 * Hero screenshot fade-in animation on first view
 */
(function initHeroAnimation() {
    const screenshot = document.querySelector('.hero-screenshot');

    if (!screenshot) return;

    const observer = new IntersectionObserver(
        (entries) => {
            entries.forEach(entry => {
                if (entry.isIntersecting) {
                    screenshot.style.opacity = '1';
                    screenshot.style.transform = 'translateY(0)';
                    observer.unobserve(screenshot);
                }
            });
        },
        { threshold: 0.3 }
    );

    screenshot.style.opacity = '0';
    screenshot.style.transform = 'translateY(20px)';
    screenshot.style.transition = 'opacity 0.6s ease, transform 0.6s ease';

    observer.observe(screenshot);
})();
