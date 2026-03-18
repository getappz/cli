<?php
/**
 * Plugin Name:       Appz Static Site Generator
 * Plugin URI:        https://github.com/getappz/wp-static-generator
 * Description:       WP-CLI commands for Simply Static — run static site generation from the command line.
 * Version:           1.0.0
 * Requires at least: 6.0
 * Requires PHP:      7.4
 * Author:            Appz
 * Author URI:        https://appz.dev
 * License:           GPL-2.0-or-later
 * License URI:       https://www.gnu.org/licenses/gpl-2.0.html
 * Text Domain:       appz-static-generator
 */

if ( ! defined( 'ABSPATH' ) ) {
	exit;
}

define( 'APPZ_SB_VERSION', '1.0.0' );
define( 'APPZ_SB_PATH', plugin_dir_path( __FILE__ ) );

// ──────────────────────────────────────────────
// Admin info page
// ──────────────────────────────────────────────

add_action( 'admin_menu', function () {
	add_management_page(
		__( 'Appz Static Site Generator', 'appz-static-generator' ),
		__( 'Appz Static Generator', 'appz-static-generator' ),
		'manage_options',
		'appz-static-generator',
		'appz_sb_render_admin_page'
	);
} );

/**
 * Render the admin info page.
 */
function appz_sb_render_admin_page() {
	$simply_static_active  = class_exists( 'Simply_Static\Plugin' );
	$simply_static_version = defined( 'SIMPLY_STATIC_VERSION' ) ? SIMPLY_STATIC_VERSION : null;

	$last_build_time   = get_option( 'appz_sb_last_build_time', null );
	$last_build_status = get_option( 'appz_sb_last_build_status', null );

	require APPZ_SB_PATH . 'views/admin-page.php';
}

// ──────────────────────────────────────────────
// WP-CLI commands
// ──────────────────────────────────────────────

if ( ! ( defined( 'WP_CLI' ) && WP_CLI ) ) {
	return;
}

/**
 * Appz static site builder — WP-CLI commands for Simply Static.
 */
class Appz_SB_CLI_Command {

	/**
	 * Run a Simply Static export synchronously.
	 *
	 * Runs all export tasks in sequence without relying on WP-Cron,
	 * making it suitable for CI/CD pipelines and automated deployments.
	 *
	 * ## OPTIONS
	 *
	 * [--output-dir=<path>]
	 * : Output directory for the static export. Overrides the Simply Static
	 *   local directory setting for this run.
	 *
	 * [--blog-id=<id>]
	 * : Blog ID for multisite. Default 0.
	 * ---
	 * default: 0
	 * ---
	 *
	 * ## EXAMPLES
	 *
	 *     wp appz build
	 *     wp appz build --output-dir=/var/www/static
	 *
	 * @when after_wp_load
	 */
	public function build( $args, $assoc_args ) {
		$this->require_simply_static();

		$blog_id = (int) ( $assoc_args['blog-id'] ?? 0 );
		$plugin  = \Simply_Static\Plugin::instance();
		$job     = $plugin->get_archive_creation_job();

		if ( $job->is_running() ) {
			WP_CLI::error( 'An export is already running. Cancel it first with: wp appz cancel' );
		}

		// ── Initialise the job ──

		\Simply_Static\Util::clear_debug_log();

		$options      = $job->get_options();
		$original_dir = null;
		$original_method = null;

		// Override output directory if specified.
		$output_dir = $assoc_args['output-dir'] ?? null;
		if ( null !== $output_dir ) {
			$output_dir = realpath( $output_dir ) ?: $output_dir;
			if ( ! is_dir( $output_dir ) ) {
				if ( ! wp_mkdir_p( $output_dir ) ) {
					WP_CLI::error( "Could not create output directory: {$output_dir}" );
				}
			}
			if ( ! is_writable( $output_dir ) ) {
				WP_CLI::error( "Output directory is not writable: {$output_dir}" );
			}
			$original_dir    = $options->get( 'local_dir' );
			$original_method = $options->get( 'delivery_method' );
			$options->set( 'local_dir', trailingslashit( $output_dir ) );
			$options->set( 'delivery_method', 'local' );
			WP_CLI::log( "Output directory: {$output_dir}" );
		}

		$task_list = $this->call_method( $job, 'get_task_list' );

		WP_CLI::log( 'Task list: ' . implode( ' → ', $task_list ) );

		$archive_name = join( '-', array( \Simply_Static\Plugin::SLUG, $blog_id, time() ) );
		$options->set( 'archive_name', $archive_name );
		$options
			->set( 'archive_status_messages', array() )
			->set( 'archive_start_time', \Simply_Static\Util::formatted_datetime() )
			->set( 'archive_end_time', null )
			->set( 'generate_type', 'export' )
			->save();

		do_action( 'ss_before_static_export' );

		// ── Run tasks synchronously ──

		$current_task_idx = 0;
		$current_task     = $task_list[0];
		$tick             = 0;

		while ( false !== $current_task ) {
			$this->call_method( $job, 'set_current_task', $current_task );
			$task_obj = $job->get_task_object( $current_task );

			if ( false === $task_obj ) {
				WP_CLI::error( "Task class not found for: {$current_task}" );
			}

			if ( 0 === $tick && method_exists( $task_obj, 'cleanup' ) ) {
				$task_obj->cleanup();
			}

			WP_CLI::log( sprintf( '[%s] Running...', $current_task ) );

			try {
				$result = $task_obj->perform();
			} catch ( \Exception $e ) {
				$this->restore_options( $options, $original_dir, $original_method );
				$this->save_build_status( 'failed' );
				WP_CLI::error( sprintf( '[%s] Exception: %s', $current_task, $e->getMessage() ) );
			}

			if ( is_wp_error( $result ) ) {
				$this->restore_options( $options, $original_dir, $original_method );
				$this->save_build_status( 'failed' );
				WP_CLI::error( sprintf( '[%s] Error: %s', $current_task, $result->get_error_message() ) );
			}

			if ( true === $result ) {
				WP_CLI::success( sprintf( '[%s] Complete.', $current_task ) );
				$current_task_idx++;
				$tick = 0;

				if ( $current_task_idx >= count( $task_list ) ) {
					$current_task = false;
				} else {
					$current_task = $task_list[ $current_task_idx ];
					$new_task_obj = $job->get_task_object( $current_task );
					if ( $new_task_obj && method_exists( $new_task_obj, 'cleanup' ) ) {
						$new_task_obj->cleanup();
					}
				}
			} else {
				$tick++;
				if ( 0 === $tick % 10 ) {
					WP_CLI::log( sprintf( '[%s] Still processing (iteration %d)...', $current_task, $tick ) );
				}
			}
		}

		// ── Finalise ──

		$options
			->set( 'archive_end_time', \Simply_Static\Util::formatted_datetime() )
			->save();

		$this->restore_options( $options, $original_dir, $original_method );

		do_action( 'ss_completed', 'success' );

		$this->save_build_status( 'success' );
		WP_CLI::success( 'Simply Static export complete.' );
	}

	/**
	 * Show Simply Static export status.
	 *
	 * Displays whether an export is running, paused, or complete,
	 * along with the current task and timing information.
	 *
	 * ## EXAMPLES
	 *
	 *     wp appz status
	 *
	 * @when after_wp_load
	 */
	public function status( $args, $assoc_args ) {
		$this->require_simply_static();

		$job     = \Simply_Static\Plugin::instance()->get_archive_creation_job();
		$options = $job->get_options();

		$running = $job->is_running() ? 'yes' : 'no';
		$paused  = $job->is_paused() ? 'yes' : 'no';
		$done    = $job->is_job_done() ? 'yes' : 'no';
		$task    = $job->get_current_task() ?: '(none)';
		$start   = $options->get( 'archive_start_time' ) ?: '(never)';
		$end     = $options->get( 'archive_end_time' ) ?: '(not finished)';

		WP_CLI::log( "Running:      {$running}" );
		WP_CLI::log( "Paused:       {$paused}" );
		WP_CLI::log( "Done:         {$done}" );
		WP_CLI::log( "Current task: {$task}" );
		WP_CLI::log( "Started:      {$start}" );
		WP_CLI::log( "Ended:        {$end}" );
	}

	/**
	 * Cancel a running Simply Static export.
	 *
	 * ## EXAMPLES
	 *
	 *     wp appz cancel
	 *
	 * @when after_wp_load
	 */
	public function cancel( $args, $assoc_args ) {
		$this->require_simply_static();

		\Simply_Static\Plugin::instance()->cancel_static_export();
		$this->save_build_status( 'cancelled' );
		WP_CLI::success( 'Export cancelled.' );
	}

	/**
	 * Ensure Simply Static is active.
	 */
	private function require_simply_static() {
		if ( ! class_exists( 'Simply_Static\Plugin' ) ) {
			WP_CLI::error( 'Simply Static plugin is not active. Install and activate it first.' );
		}
	}

	/**
	 * Call a protected method via reflection.
	 */
	private function call_method( $obj, $method_name, ...$args ) {
		$method = new \ReflectionMethod( $obj, $method_name );
		$method->setAccessible( true );
		return $method->invoke( $obj, ...$args );
	}

	/**
	 * Restore original Simply Static options if they were overridden.
	 */
	private function restore_options( $options, $original_dir, $original_method ) {
		if ( null !== $original_dir ) {
			$options->set( 'local_dir', $original_dir );
			$options->set( 'delivery_method', $original_method );
			$options->save();
		}
	}

	/**
	 * Save build time and status for the admin info page.
	 */
	private function save_build_status( $status ) {
		update_option( 'appz_sb_last_build_time', current_time( 'mysql' ), false );
		update_option( 'appz_sb_last_build_status', $status, false );
	}
}

WP_CLI::add_command( 'appz', 'Appz_SB_CLI_Command' );
