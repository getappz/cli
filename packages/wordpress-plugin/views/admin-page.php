<?php
/**
 * Admin info page template.
 *
 * @var bool        $simply_static_active
 * @var string|null $simply_static_version
 * @var string|null $last_build_time
 * @var string|null $last_build_status
 */

if ( ! defined( 'ABSPATH' ) ) {
	exit;
}
?>
<div class="wrap">
	<h1><?php esc_html_e( 'Appz Static Site Generator', 'appz-static-generator' ); ?></h1>

	<div class="card" style="max-width: 600px;">
		<h2><?php esc_html_e( 'Backend Status', 'appz-static-generator' ); ?></h2>
		<table class="widefat striped" style="border: none;">
			<tbody>
				<tr>
					<td><strong><?php esc_html_e( 'Simply Static', 'appz-static-generator' ); ?></strong></td>
					<td>
						<?php if ( $simply_static_active ) : ?>
							<span style="color: #00a32a;">&#10003;</span>
							<?php
							printf(
								/* translators: %s: plugin version */
								esc_html__( 'Active (v%s)', 'appz-static-generator' ),
								esc_html( $simply_static_version )
							);
							?>
						<?php else : ?>
							<span style="color: #d63638;">&#10007;</span>
							<?php esc_html_e( 'Not found — install and activate Simply Static', 'appz-static-generator' ); ?>
						<?php endif; ?>
					</td>
				</tr>
				<tr>
					<td><strong><?php esc_html_e( 'Last Build', 'appz-static-generator' ); ?></strong></td>
					<td>
						<?php if ( $last_build_time ) : ?>
							<?php echo esc_html( $last_build_time ); ?>
							&mdash;
							<?php
							$status_colors = array(
								'success'   => '#00a32a',
								'failed'    => '#d63638',
								'cancelled' => '#dba617',
							);
							$color = $status_colors[ $last_build_status ] ?? '#666';
							?>
							<span style="color: <?php echo esc_attr( $color ); ?>; font-weight: 600;">
								<?php echo esc_html( ucfirst( $last_build_status ) ); ?>
							</span>
						<?php else : ?>
							<?php esc_html_e( 'No builds yet', 'appz-static-generator' ); ?>
						<?php endif; ?>
					</td>
				</tr>
			</tbody>
		</table>
	</div>

	<div class="card" style="max-width: 600px;">
		<h2><?php esc_html_e( 'CLI Commands', 'appz-static-generator' ); ?></h2>
		<table class="widefat striped" style="border: none;">
			<thead>
				<tr>
					<th><?php esc_html_e( 'Command', 'appz-static-generator' ); ?></th>
					<th><?php esc_html_e( 'Description', 'appz-static-generator' ); ?></th>
				</tr>
			</thead>
			<tbody>
				<tr>
					<td><code>wp appz build</code></td>
					<td><?php esc_html_e( 'Run a full static site export', 'appz-static-generator' ); ?></td>
				</tr>
				<tr>
					<td><code>wp appz build --output-dir=&lt;path&gt;</code></td>
					<td><?php esc_html_e( 'Export to a specific directory', 'appz-static-generator' ); ?></td>
				</tr>
				<tr>
					<td><code>wp appz status</code></td>
					<td><?php esc_html_e( 'Show current export status', 'appz-static-generator' ); ?></td>
				</tr>
				<tr>
					<td><code>wp appz cancel</code></td>
					<td><?php esc_html_e( 'Cancel a running export', 'appz-static-generator' ); ?></td>
				</tr>
			</tbody>
		</table>
	</div>
</div>
